use crate::{
    assemble::resolution::{ResolvedAddress, ResolvedInstruction, ResolvedOperand, Resolver},
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, DiagnosticLabel, Partial},
    frontend::{
        analysis::symbol_table::SymbolTable,
        syntax::statements::{
            DirectiveArg, DirectiveStatement, InstructionStatement, Operand, Program, Statement,
        },
    },
};

const PAGE_SIZE_BYTES: i64 = 128;

#[derive(Debug, Clone, Default)]
pub struct PageChecker;

impl PageChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze(&self, program: &Program, symbols: &SymbolTable) -> Partial<()> {
        let mut emitter = DiagnosticEmitter::new();
        let resolver = Resolver::new();
        let mut cursor = 0i64;
        let mut current_page_start = None::<i64>;

        for statement in &program.statements {
            match statement {
                Statement::Label(_) => {}
                Statement::Instruction(instruction) => {
                    let instruction_start = cursor;

                    if let Some(page_start) = current_page_start {
                        if instruction_start + 2 > page_start + PAGE_SIZE_BYTES {
                            emitter.push(page_error(
                                instruction.span,
                                "instruction exceeds the 64-instruction page",
                            ));
                        }
                    }

                    if instruction.mnemonic == "bra" {
                        match resolver.resolve_instruction(instruction, symbols) {
                            Ok(resolved) => {
                                if let Some(diagnostic) = validate_branch_target(
                                    program,
                                    instruction,
                                    instruction_start,
                                    &resolved,
                                    symbols,
                                ) {
                                    emitter.push(diagnostic);
                                }
                            }
                            Err(diagnostic) => emitter.push(diagnostic),
                        }
                    }

                    cursor += 2;
                }
                Statement::Directive(directive) => match directive.name.as_str() {
                    "page" => {
                        let Some(page) = directive_int(directive, 0, &mut emitter) else {
                            continue;
                        };
                        if page < 0 {
                            emitter.push(page_error(
                                directive.span,
                                "directive `.page` expects a non-negative page number",
                            ));
                            continue;
                        }

                        cursor = page << 7;
                        current_page_start = Some(cursor);
                    }
                    "org" => {
                        let Some(target) = directive_int(directive, 0, &mut emitter) else {
                            continue;
                        };
                        if target < 0 {
                            emitter.push(page_error(
                                directive.span,
                                "directive `.org` expects a non-negative address",
                            ));
                            continue;
                        }

                        cursor = target;
                        current_page_start = None;
                    }
                    "bytes" => {
                        if let Some(page_start) = current_page_start {
                            let next = cursor + directive.args.len() as i64;
                            if next > page_start + PAGE_SIZE_BYTES {
                                emitter.push(page_error(
                                    directive.span,
                                    "directive `.bytes` exceeds the current page",
                                ));
                            }
                        }
                        cursor += directive.args.len() as i64;
                    }
                    "string" => match directive.args.first() {
                        Some(DirectiveArg::String(value)) => {
                            if let Some(page_start) = current_page_start {
                                let next = cursor + value.len() as i64;
                                if next > page_start + PAGE_SIZE_BYTES {
                                    emitter.push(page_error(
                                        directive.span,
                                        "directive `.string` exceeds the current page",
                                    ));
                                }
                            }
                            cursor += value.len() as i64;
                        }
                        _ => emitter.push(page_error(
                            directive.span,
                            "directive `.string` expects a string argument",
                        )),
                    },
                    "zero" => {
                        let Some(count) = directive_int(directive, 0, &mut emitter) else {
                            continue;
                        };
                        if count < 0 {
                            emitter.push(page_error(
                                directive.span,
                                "directive `.zero` expects a non-negative count",
                            ));
                            continue;
                        }
                        if let Some(page_start) = current_page_start {
                            let next = cursor + count;
                            if next > page_start + PAGE_SIZE_BYTES {
                                emitter.push(page_error(
                                    directive.span,
                                    "directive `.zero` exceeds the current page",
                                ));
                            }
                        }
                        cursor += count;
                    }
                    _ => {}
                },
            }
        }

        emitter.finish(())
    }
}

fn validate_branch_target(
    program: &Program,
    instruction: &InstructionStatement,
    instruction_start: i64,
    resolved: &ResolvedInstruction,
    symbols: &SymbolTable,
) -> Option<Diagnostic> {
    let Some(target) = resolved
        .operands
        .first()
        .and_then(branch_target_byte_address)
    else {
        return Some(page_error(
            resolved.span,
            "branch target must resolve to a byte address",
        ));
    };

    if target % 2 != 0 {
        return Some(page_error(
            resolved.span,
            "branch target must be instruction-aligned",
        ));
    }

    if instruction_start.div_euclid(PAGE_SIZE_BYTES) != target.div_euclid(PAGE_SIZE_BYTES) {
        let source_page = instruction_start.div_euclid(PAGE_SIZE_BYTES);
        let mut diagnostic = Diagnostic::error_code(DiagnosticCode::EncodingError(
            "branch target crosses a 64-instruction page boundary".to_owned(),
        ))
        .with_label(DiagnosticLabel::new(
            resolved.span,
            format!("branch is in page {source_page}.."),
        ));
        if let Some(target_label) = branch_target_label(instruction, symbols) {
            if let Some((page_directive, page_number)) =
                enclosing_page_directive(program, &target_label.name)
            {
                let _ = page_directive;
                diagnostic.push_label(DiagnosticLabel::secondary(
                    target_label.span,
                    format!(
                        "..but target label `{}` is in page {page_number}",
                        target_label.name
                    ),
                ));
            } else {
                let target_page = target.div_euclid(PAGE_SIZE_BYTES);
                diagnostic.push_label(DiagnosticLabel::secondary(
                    target_label.span,
                    format!(
                        "..but target label `{}` is in page {target_page}",
                        target_label.name
                    ),
                ));
            }
        }
        return Some(diagnostic);
    }

    None
}

fn branch_target_label<'a>(
    instruction: &InstructionStatement,
    symbols: &'a SymbolTable,
) -> Option<&'a crate::frontend::analysis::symbol_table::Symbol> {
    let Operand::Label(name) = instruction.operands.first()? else {
        return None;
    };
    symbols.get(name)
}

fn branch_target_byte_address(operand: &ResolvedOperand) -> Option<i64> {
    match operand {
        ResolvedOperand::Immediate(value) => Some(*value),
        ResolvedOperand::Label(label) => Some(label.value),
        ResolvedOperand::Address(ResolvedAddress::Direct(address)) => Some(*address),
        ResolvedOperand::Register(_)
        | ResolvedOperand::Address(ResolvedAddress::Pointer { .. })
        | ResolvedOperand::Condition(_) => None,
    }
}

fn enclosing_page_directive<'a>(
    program: &'a Program,
    target_label: &str,
) -> Option<(&'a DirectiveStatement, i64)> {
    let mut current_page = None;

    for statement in &program.statements {
        match statement {
            Statement::Directive(directive) if directive.name == "page" => {
                let Some(page) = directive.args.first() else {
                    continue;
                };
                let page_number = match page {
                    DirectiveArg::Integer { value, .. } | DirectiveArg::Char { value, .. } => {
                        *value
                    }
                    DirectiveArg::Identifier(_) | DirectiveArg::String(_) => continue,
                };
                current_page = Some((directive, page_number));
            }
            Statement::Label(label) if label.name == target_label => return current_page,
            Statement::Label(_) | Statement::Instruction(_) | Statement::Directive(_) => {}
        }
    }

    None
}

fn directive_int(
    directive: &DirectiveStatement,
    index: usize,
    emitter: &mut DiagnosticEmitter,
) -> Option<i64> {
    match directive.args.get(index) {
        Some(DirectiveArg::Integer { value, .. } | DirectiveArg::Char { value, .. }) => {
            Some(*value)
        }
        _ => {
            emitter.push(page_error(
                directive.span,
                format!(
                    "directive `.{}` expects an integer argument",
                    directive.name
                ),
            ));
            None
        }
    }
}

fn page_error(span: crate::diagnostics::Span, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    Diagnostic::error_code(DiagnosticCode::EncodingError(message.clone()))
        .with_label(DiagnosticLabel::new(span, message))
}

#[cfg(test)]
mod tests {
    use crate::{
        assemble::page_checker::PageChecker,
        frontend::{analysis::symbol_table::SymbolTable, syntax::parser::Parser},
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
    fn accepts_same_page_branches() {
        let program = parse(".page 0\nstart:\nhalt\nbra start, ?equal\n");
        let symbols = SymbolTable::build(&program).into_result().unwrap();

        PageChecker::new()
            .analyze(&program, &symbols)
            .into_result()
            .unwrap();
    }

    #[test]
    fn rejects_cross_page_branches() {
        let program = parse(".page 0\nstart:\nbra done, ?equal\n.page 1\ndone:\nhalt\n");
        let symbols = SymbolTable::build(&program).into_result().unwrap();
        let errors = PageChecker::new()
            .analyze(&program, &symbols)
            .into_result()
            .unwrap_err();

        assert_eq!(
            errors[0].message,
            "branch target crosses a 64-instruction page boundary"
        );
        assert_eq!(errors[0].labels.len(), 2);
        assert_eq!(errors[0].labels[0].message, "branch is in page 0..");
        assert_eq!(
            errors[0].labels[1].message,
            "..but target label `done` is in page 1"
        );
    }

    #[test]
    fn rejects_oversize_pages() {
        let mut source = String::from(".page 0\n");
        for _ in 0..65 {
            source.push_str("halt\n");
        }

        let program = parse(&source);
        let symbols = SymbolTable::build(&program).into_result().unwrap();
        let errors = PageChecker::new()
            .analyze(&program, &symbols)
            .into_result()
            .unwrap_err();

        assert_eq!(
            errors[0].message,
            "instruction exceeds the 64-instruction page"
        );
    }
}
