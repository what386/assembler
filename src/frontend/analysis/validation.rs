use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, DiagnosticLabel, Partial},
    frontend::{
        analysis::{
            isa::{InstructionSpec, OpFormatKind, lookup_instruction},
            symbol_table::SymbolTable,
        },
        syntax::statements::{
            Address, DirectiveArg, DirectiveStatement, InstructionStatement, Operand, Program,
            Statement,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedOperand {
    Register,
    Condition,
    Immediate,
    Address,
    IndexedAddress { allow_offset: bool },
    Location,
}

#[derive(Debug, Clone, Default)]
pub struct Validator;

impl Validator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_program(&self, program: &Program) -> Partial<()> {
        let mut emitter = DiagnosticEmitter::new();
        emitter.extend(SymbolTable::build(program).diagnostics);

        for statement in &program.statements {
            match statement {
                Statement::Instruction(instruction) => {
                    if let Err(diagnostic) = self.validate_instruction(instruction) {
                        emitter.push(diagnostic);
                    }
                }
                Statement::Directive(directive) => {
                    if let Err(diagnostic) = self.validate_directive(directive) {
                        emitter.push(diagnostic);
                    }
                }
                Statement::Label(_) => {}
            }
        }

        emitter.finish(())
    }

    pub fn validate_instruction(
        &self,
        instruction: &InstructionStatement,
    ) -> Result<(), Diagnostic> {
        let spec = lookup_instruction(&instruction.mnemonic).ok_or_else(|| {
            self.instruction_error(
                instruction,
                format!("unknown instruction `{}`", instruction.mnemonic),
            )
        })?;

        let expected = expected_operands(instruction.mnemonic.as_str(), spec);
        self.expect_instruction_shape(instruction, &expected)
    }

    fn validate_directive(&self, directive: &DirectiveStatement) -> Result<(), Diagnostic> {
        match directive.name.as_str() {
            "section" => {
                if matches!(directive.args.first(), Some(DirectiveArg::Identifier(_))) {
                    Ok(())
                } else {
                    Err(self.directive_error(directive, "expected section name"))
                }
            }
            "page" | "org" | "zero" => {
                if matches!(
                    directive.args.first(),
                    Some(DirectiveArg::Integer { .. } | DirectiveArg::Char { .. })
                ) {
                    Ok(())
                } else {
                    Err(self.directive_error(directive, "expected integer argument"))
                }
            }
            "string" => {
                if matches!(directive.args.first(), Some(DirectiveArg::String(_))) {
                    Ok(())
                } else {
                    Err(self.directive_error(directive, "expected string argument"))
                }
            }
            "bytes" => {
                if directive.args.iter().all(|arg| {
                    matches!(
                        arg,
                        DirectiveArg::Integer { .. } | DirectiveArg::Char { .. }
                    )
                }) {
                    Ok(())
                } else {
                    Err(self.directive_error(directive, "expected only byte-sized literals"))
                }
            }
            _ => Ok(()),
        }
    }

    fn expect_instruction_shape(
        &self,
        instruction: &InstructionStatement,
        expected: &[ExpectedOperand],
    ) -> Result<(), Diagnostic> {
        if instruction.operands.len() != expected.len() {
            return Err(self.instruction_error(
                instruction,
                expected_operand_count_message(instruction.mnemonic.as_str(), expected),
            ));
        }

        for (index, (operand, expected_operand)) in
            instruction.operands.iter().zip(expected.iter()).enumerate()
        {
            if operand_matches(operand, *expected_operand) {
                continue;
            }

            if let Some(message) =
                special_operand_error(instruction.mnemonic.as_str(), index, *expected_operand)
            {
                return Err(self.instruction_error(instruction, message));
            }

            if matches!(expected_operand, ExpectedOperand::IndexedAddress { .. })
                && matches!(operand, Operand::Address(Address::Absolute(_)))
            {
                return Err(
                    self.instruction_error(instruction, "instruction requires an indexed address")
                );
            }

            return Err(self.instruction_error(
                instruction,
                format!(
                    "{} operand must be {}",
                    ordinal(index),
                    expected_operand.description()
                ),
            ));
        }

        Ok(())
    }

    fn instruction_error(
        &self,
        instruction: &InstructionStatement,
        message: impl Into<String>,
    ) -> Diagnostic {
        let message = message.into();
        Diagnostic::error_code(DiagnosticCode::InvalidOperand(message.clone()))
            .with_label(DiagnosticLabel::new(instruction.span, message))
    }

    fn directive_error(
        &self,
        directive: &DirectiveStatement,
        message: impl Into<String>,
    ) -> Diagnostic {
        let message = message.into();
        Diagnostic::error_code(DiagnosticCode::InvalidDirective(message.clone()))
            .with_label(DiagnosticLabel::new(directive.span, message))
    }
}

fn expected_operands(mnemonic: &str, spec: InstructionSpec) -> Vec<ExpectedOperand> {
    spec.operand_formats()
        .into_iter()
        .enumerate()
        .map(|(index, operand)| match operand.kind {
            OpFormatKind::Register => ExpectedOperand::Register,
            OpFormatKind::Condition => ExpectedOperand::Condition,
            OpFormatKind::Address => ExpectedOperand::Address,
            OpFormatKind::Pointer => ExpectedOperand::IndexedAddress {
                allow_offset: false,
            },
            OpFormatKind::OffsetPointer => ExpectedOperand::IndexedAddress { allow_offset: true },
            OpFormatKind::Immediate { .. } | OpFormatKind::Offset { .. }
                if is_location_operand(mnemonic, index) =>
            {
                ExpectedOperand::Location
            }
            OpFormatKind::Immediate { .. } | OpFormatKind::Offset { .. } => {
                ExpectedOperand::Immediate
            }
        })
        .collect()
}

fn is_location_operand(mnemonic: &str, index: usize) -> bool {
    matches!((mnemonic, index), ("jmp" | "cal" | "bra", 0))
}

fn operand_matches(operand: &Operand, expected: ExpectedOperand) -> bool {
    match expected {
        ExpectedOperand::Register => matches!(operand, Operand::Register(_)),
        ExpectedOperand::Condition => matches!(operand, Operand::Condition(_)),
        ExpectedOperand::Immediate => matches!(operand, Operand::Immediate(_)),
        ExpectedOperand::Address => matches!(operand, Operand::Address(_)),
        ExpectedOperand::IndexedAddress { allow_offset } => {
            matches!(
                operand,
                Operand::Address(Address::Indexed { offset: None, .. })
            ) || (allow_offset
                && matches!(
                    operand,
                    Operand::Address(Address::Indexed {
                        offset: Some(_),
                        ..
                    })
                ))
        }
        ExpectedOperand::Location => matches!(operand, Operand::Label(_) | Operand::Address(_)),
    }
}

fn expected_operand_count_message(mnemonic: &str, expected: &[ExpectedOperand]) -> String {
    match (mnemonic, expected) {
        ("jmp" | "cal", [ExpectedOperand::Location]) => "expected one location operand".to_owned(),
        ("bra", [ExpectedOperand::Location, ExpectedOperand::Condition]) => {
            "expected location and condition".to_owned()
        }
        _ => format!("expected {} operand(s)", expected.len()),
    }
}

fn special_operand_error(
    mnemonic: &str,
    index: usize,
    expected: ExpectedOperand,
) -> Option<&'static str> {
    match (mnemonic, index, expected) {
        ("bra", 0, ExpectedOperand::Location) => {
            Some("first branch operand must be a label or address")
        }
        ("bra", 1, ExpectedOperand::Condition) => Some("second branch operand must be a condition"),
        ("jmp" | "cal", 0, ExpectedOperand::Location) => Some("expected label or address operand"),
        _ => None,
    }
}

fn ordinal(index: usize) -> &'static str {
    match index {
        0 => "first",
        1 => "second",
        2 => "third",
        3 => "fourth",
        _ => "next",
    }
}

impl ExpectedOperand {
    fn description(self) -> &'static str {
        match self {
            Self::Register => "a register",
            Self::Condition => "a condition",
            Self::Immediate => "an immediate",
            Self::Address => "an address",
            Self::IndexedAddress { .. } => "an indexed address",
            Self::Location => "a label or address",
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        frontend::{analysis::validation::Validator, syntax::parser::Parser},
        preprocessing::Preprocessor,
    };

    fn parse(source: &str) -> crate::frontend::syntax::statements::Program {
        let preprocessed = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap();
        Parser::new(&preprocessed.tokens)
            .parse()
            .into_result()
            .unwrap()
    }

    #[test]
    fn validates_known_instructions() {
        let program = parse("start:\nlim r0, 1\nmlx r2, [r3+4]\nbra start, ?equal\n");
        Validator::new()
            .validate_program(&program)
            .into_result()
            .unwrap();
    }

    #[test]
    fn validates_alias_and_pseudo_instructions() {
        let program = parse("mov r7, r6, ?always\npeek r2, 4\ncmp r1, r0\ninc r3\n");
        Validator::new()
            .validate_program(&program)
            .into_result()
            .unwrap();
    }

    #[test]
    fn validates_func_and_ctrl_leaf_instructions() {
        let program = parse("halt\nnop\nint 3\nmpge 1\ntimer.init 0x10\npcw.set 2\n");
        Validator::new()
            .validate_program(&program)
            .into_result()
            .unwrap();
    }

    #[test]
    fn rejects_invalid_branch_shape() {
        let program = parse("bra ?equal, loop\n");
        let errors = Validator::new()
            .validate_program(&program)
            .into_result()
            .unwrap_err();

        assert_eq!(
            errors[0].message,
            "first branch operand must be a label or address"
        );
    }

    #[test]
    fn rejects_unknown_instruction() {
        let program = parse("wat r0\n");
        let errors = Validator::new()
            .validate_program(&program)
            .into_result()
            .unwrap_err();

        assert_eq!(errors[0].message, "unknown instruction `wat`");
    }

    #[test]
    fn rejects_raw_family_and_dotted_compat_names() {
        for source in ["func 0\n", "ctrl 0\n", "func.halt\n"] {
            let program = parse(source);
            let errors = Validator::new()
                .validate_program(&program)
                .into_result()
                .unwrap_err();

            assert!(errors[0].message.starts_with("unknown instruction `"));
        }
    }

    #[test]
    fn collects_multiple_validation_errors() {
        let program = parse("wat r0\nbra ?equal, loop\n.org text\n");
        let errors = Validator::new()
            .validate_program(&program)
            .into_result()
            .unwrap_err();

        assert_eq!(errors.len(), 4);
        assert_eq!(
            errors[0].message,
            "directive `.org` expected integer argument"
        );
        assert_eq!(errors[1].message, "unknown instruction `wat`");
        assert_eq!(
            errors[2].message,
            "first branch operand must be a label or address"
        );
        assert_eq!(errors[3].message, "expected integer argument");
    }
}
