use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, DiagnosticLabel, Partial, Span},
    frontend::{
        analysis::{isa::lookup_instruction, symbol_table::SymbolTable},
        syntax::statements::{
            Address, AltCondition, Condition, InstructionStatement, Operand, Program, Register,
            Statement, StdCondition,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedInstruction {
    pub mnemonic: String,
    pub kind: Option<u8>,
    pub operands: Vec<ResolvedOperand>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedOperand {
    Register(i64),
    Immediate(i64),
    Address(ResolvedAddress),
    Condition(ResolvedCondition),
    Label(ResolvedLabel),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedAddress {
    Direct(i64),
    Pointer { register: i64, offset: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLabel {
    pub name: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedCondition {
    Standard(ResolvedStdCondition),
    Alternate(ResolvedAltCondition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedStdCondition {
    Equal,
    NotEqual,
    Lower,
    Higher,
    LowerSame,
    HigherSame,
    Even,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedAltCondition {
    Overflow,
    NoOverflow,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Odd,
    Always,
}

#[derive(Debug, Clone, Default)]
pub struct Resolver;

impl Resolver {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve_program(
        &self,
        program: &Program,
        symbols: &SymbolTable,
    ) -> Partial<Vec<ResolvedInstruction>> {
        let mut resolved = Vec::new();
        let mut emitter = DiagnosticEmitter::new();

        for statement in &program.statements {
            let Statement::Instruction(instruction) = statement else {
                continue;
            };

            match self.resolve_instruction(instruction, symbols) {
                Ok(instruction) => resolved.push(instruction),
                Err(diagnostic) => emitter.push(diagnostic),
            }
        }

        emitter.finish(resolved)
    }

    pub fn resolve_instruction(
        &self,
        instruction: &InstructionStatement,
        symbols: &SymbolTable,
    ) -> Result<ResolvedInstruction, Diagnostic> {
        let spec = lookup_instruction(&instruction.mnemonic).ok_or_else(|| {
            Diagnostic::error_code(DiagnosticCode::InvalidOperand(format!(
                "unknown instruction `{}`",
                instruction.mnemonic
            )))
            .with_label(DiagnosticLabel::new(
                instruction.span,
                format!("`{}` is not a known instruction", instruction.mnemonic),
            ))
        })?;
        let mnemonic = spec.resolved_mnemonic.to_owned();
        let kind = spec.kind;

        let operands = instruction
            .operands
            .iter()
            .map(|operand| self.resolve_operand(operand, instruction.span, symbols))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ResolvedInstruction {
            mnemonic,
            kind,
            operands,
            span: instruction.span,
        })
    }

    fn resolve_operand(
        &self,
        operand: &Operand,
        span: Span,
        symbols: &SymbolTable,
    ) -> Result<ResolvedOperand, Diagnostic> {
        match operand {
            Operand::Register(register) => {
                Ok(ResolvedOperand::Register(resolve_register(register)))
            }
            Operand::Immediate(value) => Ok(ResolvedOperand::Immediate(*value)),
            Operand::Address(address) => Ok(ResolvedOperand::Address(resolve_address(address))),
            Operand::Condition(condition) => {
                Ok(ResolvedOperand::Condition(resolve_condition(condition)))
            }
            Operand::Label(name) => {
                let symbol = symbols.get(name).ok_or_else(|| {
                    Diagnostic::error_code(DiagnosticCode::UnexpectedToken(format!(
                        "unknown label `{name}`"
                    )))
                    .with_label(DiagnosticLabel::new(
                        span,
                        format!("`{name}` is not defined"),
                    ))
                })?;

                Ok(ResolvedOperand::Label(ResolvedLabel {
                    name: symbol.name.clone(),
                    value: symbol.value,
                }))
            }
        }
    }
}

fn resolve_register(register: &Register) -> i64 {
    match register {
        Register::R0 => 0,
        Register::R1 => 1,
        Register::R2 => 2,
        Register::R3 => 3,
        Register::R4 => 4,
        Register::R5 => 5,
        Register::R6 => 6,
        Register::R7 => 7,
    }
}

fn resolve_address(address: &Address) -> ResolvedAddress {
    match address {
        Address::Absolute(value) => ResolvedAddress::Direct(*value as i64),
        Address::Indexed { base, offset } => ResolvedAddress::Pointer {
            register: resolve_register(base),
            offset: i64::from(offset.unwrap_or(0)),
        },
    }
}

fn resolve_condition(condition: &Condition) -> ResolvedCondition {
    match condition {
        Condition::Standard(condition) => ResolvedCondition::Standard(match condition {
            StdCondition::Equal => ResolvedStdCondition::Equal,
            StdCondition::NotEqual => ResolvedStdCondition::NotEqual,
            StdCondition::Lower => ResolvedStdCondition::Lower,
            StdCondition::Higher => ResolvedStdCondition::Higher,
            StdCondition::LowerSame => ResolvedStdCondition::LowerSame,
            StdCondition::HigherSame => ResolvedStdCondition::HigherSame,
            StdCondition::Even => ResolvedStdCondition::Even,
            StdCondition::Always => ResolvedStdCondition::Always,
        }),
        Condition::Alternate(condition) => ResolvedCondition::Alternate(match condition {
            AltCondition::Overflow => ResolvedAltCondition::Overflow,
            AltCondition::NoOverflow => ResolvedAltCondition::NoOverflow,
            AltCondition::Less => ResolvedAltCondition::Less,
            AltCondition::Greater => ResolvedAltCondition::Greater,
            AltCondition::LessEqual => ResolvedAltCondition::LessEqual,
            AltCondition::GreaterEqual => ResolvedAltCondition::GreaterEqual,
            AltCondition::Odd => ResolvedAltCondition::Odd,
            AltCondition::Always => ResolvedAltCondition::Always,
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        frontend::{
            analysis::{resolution::Resolver, symbol_table::SymbolTable},
            syntax::parser::Parser,
        },
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
    fn resolves_labels_and_conditions() {
        let program = parse("start:\nbra start, ?equal\n");
        let symbols = SymbolTable::build(&program).into_result().unwrap();
        let resolved = Resolver::new()
            .resolve_program(&program, &symbols)
            .into_result()
            .unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].mnemonic, "bra");
        assert_eq!(resolved[0].kind, None);
        assert!(matches!(
            resolved[0].operands[0],
            super::ResolvedOperand::Label(super::ResolvedLabel { value: 0, .. })
        ));
        assert!(matches!(
            resolved[0].operands[1],
            super::ResolvedOperand::Condition(super::ResolvedCondition::Standard(
                super::ResolvedStdCondition::Equal
            ))
        ));
    }

    #[test]
    fn canonicalizes_aliases_and_preserves_pseudos() {
        let program = parse("start:\nmov r1, r2, ?always\nadd r3, r4, r5\ncmp r6, r7\nhalt\n");
        let symbols = SymbolTable::build(&program).into_result().unwrap();
        let resolved = Resolver::new()
            .resolve_program(&program, &symbols)
            .into_result()
            .unwrap();

        assert_eq!(resolved[0].mnemonic, "cmov");
        assert_eq!(resolved[0].kind, Some(0));
        assert_eq!(resolved[1].mnemonic, "add");
        assert_eq!(resolved[1].kind, Some(0));
        assert_eq!(resolved[2].mnemonic, "cmp");
        assert_eq!(resolved[2].kind, None);
        assert_eq!(resolved[3].mnemonic, "halt");
        assert_eq!(resolved[3].kind, None);
    }

    #[test]
    fn resolves_indexed_addresses_and_alternate_conditions() {
        let program = parse("start:\nmsx r2, [r3-4]\nbra start, @greater_equal\n");
        let symbols = SymbolTable::build(&program).into_result().unwrap();
        let resolved = Resolver::new()
            .resolve_program(&program, &symbols)
            .into_result()
            .unwrap();

        assert!(matches!(
            resolved[0].operands[1],
            super::ResolvedOperand::Address(super::ResolvedAddress::Pointer {
                register: 3,
                offset: -4,
            })
        ));
        assert!(matches!(
            resolved[1].operands[1],
            super::ResolvedOperand::Condition(super::ResolvedCondition::Alternate(
                super::ResolvedAltCondition::GreaterEqual
            ))
        ));
    }

    #[test]
    fn resolves_func_leaf_aliases_and_pseudos() {
        let program = parse("halt\nint 3\n");
        let symbols = SymbolTable::build(&program).into_result().unwrap();
        let resolved = Resolver::new()
            .resolve_program(&program, &symbols)
            .into_result()
            .unwrap();

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].mnemonic, "halt");
        assert_eq!(resolved[0].kind, None);
        assert!(resolved[0].operands.is_empty());
        assert_eq!(resolved[1].mnemonic, "func");
        assert_eq!(resolved[1].kind, Some(0b111));
        assert!(matches!(
            resolved[1].operands[0],
            super::ResolvedOperand::Immediate(3)
        ));
    }

    #[test]
    fn reports_unknown_label_during_resolution() {
        let program = parse("jmp missing\n");
        let symbols = SymbolTable::build(&program).into_result().unwrap();
        let errors = Resolver::new()
            .resolve_program(&program, &symbols)
            .into_result()
            .unwrap_err();

        assert_eq!(errors[0].message, "unknown label `missing`");
        assert_eq!(errors[0].labels[0].message, "`missing` is not defined");
    }

    #[test]
    fn collects_multiple_resolution_errors() {
        let program = parse("jmp missing\ncal absent\nhalt\n");
        let symbols = SymbolTable::build(&program).into_result().unwrap();
        let resolved = Resolver::new().resolve_program(&program, &symbols);

        assert_eq!(resolved.diagnostics.len(), 2);
        assert_eq!(resolved.value.unwrap().len(), 1);
    }
}
