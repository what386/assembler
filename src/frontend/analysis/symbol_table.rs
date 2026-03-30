use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, DiagnosticLabel, Partial, Span},
    frontend::syntax::statements::{DirectiveArg, DirectiveStatement, Program, Statement},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub value: i64,
    pub span: Span,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTable {
    pub labels: Vec<Symbol>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(program: &Program) -> Partial<Self> {
        let mut table = Self::new();
        let mut location = 0_i64;
        let mut emitter = DiagnosticEmitter::new();

        for statement in &program.statements {
            match statement {
                Statement::Label(label) => {
                    if let Some(existing) = table.get(&label.name) {
                        emitter.push(
                            Diagnostic::error_code(DiagnosticCode::InvalidDirective(format!(
                                "duplicate label `{}`",
                                label.name
                            )))
                            .with_label(DiagnosticLabel::new(
                                label.span,
                                format!("`{}` redefined here", label.name),
                            ))
                            .with_label(DiagnosticLabel::secondary(
                                existing.span,
                                format!("previous definition of `{}`", label.name),
                            )),
                        );
                        continue;
                    }

                    table.labels.push(Symbol {
                        name: label.name.clone(),
                        value: location,
                        span: label.span,
                    });
                }
                Statement::Instruction(_) => {
                    location += 2;
                }
                Statement::Directive(directive) => {
                    match apply_directive_location(location, directive) {
                        Ok(next_location) => location = next_location,
                        Err(diagnostic) => emitter.push(diagnostic),
                    }
                }
            }
        }

        emitter.finish(table)
    }

    pub fn get(&self, name: &str) -> Option<&Symbol> {
        self.labels.iter().find(|symbol| symbol.name == name)
    }
}

fn apply_directive_location(
    current: i64,
    directive: &DirectiveStatement,
) -> Result<i64, Diagnostic> {
    match directive.name.as_str() {
        "section" => Ok(current),
        "page" => {
            let page = expect_integer_arg(directive, 0)?;
            Ok(page << 7)
        }
        "org" => expect_integer_arg(directive, 0),
        "bytes" => Ok(current + directive.args.len() as i64),
        "string" => match directive.args.first() {
            Some(DirectiveArg::String(value)) => Ok(current + value.len() as i64),
            _ => Err(directive_error(directive, "expected string argument")),
        },
        "zero" => {
            let count = expect_integer_arg(directive, 0)?;
            Ok(current + count)
        }
        _ => Ok(current),
    }
}

fn expect_integer_arg(directive: &DirectiveStatement, index: usize) -> Result<i64, Diagnostic> {
    match directive.args.get(index) {
        Some(DirectiveArg::Integer { value, .. }) => Ok(*value),
        Some(DirectiveArg::Char { value, .. }) => Ok(*value),
        _ => Err(directive_error(directive, "expected integer argument")),
    }
}

fn directive_error(directive: &DirectiveStatement, message: &str) -> Diagnostic {
    Diagnostic::error_code(DiagnosticCode::InvalidDirective(format!(
        "directive `.{}` {message}",
        directive.name
    )))
    .with_label(DiagnosticLabel::new(directive.span, message))
}

#[cfg(test)]
mod tests {
    use crate::{
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
    fn builds_labels_from_instructions_and_directives() {
        let program = parse(".page 1\nstart:\nlim r0, 1\n.org 0x20\ndata:\n.bytes 0x01, 0x02\n");
        let table = SymbolTable::build(&program).into_result().unwrap();

        assert_eq!(table.get("start").unwrap().value, 128);
        assert_eq!(table.get("data").unwrap().value, 32);
    }
}
