use crate::{
    directives::{data::directive_data_len, incbin::{IncbinContext, incbin_length}},
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
        Self::build_with_context(program, &IncbinContext::default())
    }

    pub fn build_for_validation(program: &Program) -> Partial<Self> {
        Self::build_inner(program, None)
    }

    pub fn build_with_context(program: &Program, incbin: &IncbinContext) -> Partial<Self> {
        Self::build_inner(program, Some(incbin))
    }

    fn build_inner(program: &Program, incbin: Option<&IncbinContext>) -> Partial<Self> {
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
                    match apply_directive_location(location, directive, incbin) {
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
    incbin: Option<&IncbinContext>,
) -> Result<i64, Diagnostic> {
    match directive.name.as_str() {
        "page" => {
            let page = expect_integer_arg(directive, 0)?;
            Ok(page << 7)
        }
        "org" => expect_integer_arg(directive, 0),
        "bytes" | "string" | "cstring" | "fill" => match directive_data_len(directive) {
            Some(Ok(length)) => Ok(current + length),
            Some(Err(diagnostic)) => Err(diagnostic),
            None => Ok(current),
        },
        "incbin" => match incbin {
            Some(incbin) => Ok(current + incbin_length(directive, incbin)?),
            None => Ok(current),
        },
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
    use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

    use crate::{
        directives::incbin::IncbinContext,
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

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("assembler-{name}-{unique}.bin"));
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn builds_labels_from_instructions_and_directives() {
        let program = parse(".page 1\nstart:\nlim r0, 1\n.org 0x20\ndata:\n.bytes 0x01, 0x02\n");
        let table = SymbolTable::build(&program).into_result().unwrap();

        assert_eq!(table.get("start").unwrap().value, 128);
        assert_eq!(table.get("data").unwrap().value, 32);
    }

    #[test]
    fn counts_incbin_bytes_in_label_locations() {
        let path = temp_file("symbol-table", &[0xaa, 0xbb, 0xcc]);
        let source = format!(".incbin \"{}\"\nafter:\nhalt\n", path.display());
        let program = parse(&source);
        let table = SymbolTable::build_with_context(&program, &IncbinContext::default())
            .into_result()
            .unwrap();

        assert_eq!(table.get("after").unwrap().value, 3);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn resolves_relative_incbin_paths_from_input_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!("assembler-rel-{unique}"));
        fs::create_dir_all(&base_dir).unwrap();
        let asset_path = base_dir.join("asset.bin");
        fs::write(&asset_path, [1_u8, 2, 3, 4]).unwrap();

        let program = parse(".incbin \"asset.bin\"\nafter:\n");
        let context = IncbinContext::from_input_path(Some(
            base_dir.join("program.asm").to_str().unwrap(),
        ));
        let table = SymbolTable::build_with_context(&program, &context)
            .into_result()
            .unwrap();

        assert_eq!(table.get("after").unwrap().value, 4);

        let _ = fs::remove_file(asset_path);
        let _ = fs::remove_dir(base_dir);
    }
}
