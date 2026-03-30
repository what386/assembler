use crate::{
    diagnostics::{DiagnosticEmitter, FileId, Partial, Span},
    frontend::syntax::{lexer::Tokenizer, tokens::Token},
};

use crate::preprocessing::{
    context::{Conditionals, Defines, current_active, unterminated_conditional_diagnostics},
    directives::{expand_defines, handle_directive, parse_preprocessor_directive},
    source_file::{comment_start, iterate_lines, mask_text},
};

#[derive(Debug, Clone)]
pub struct PreprocessedSource {
    pub masked_source: String,
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone, Default)]
pub struct Preprocessor;

impl Preprocessor {
    pub fn new() -> Self {
        Self
    }

    pub fn preprocess(&self, file_id: FileId, source: &str) -> Partial<PreprocessedSource> {
        self.preprocess_with_defines(file_id, source, &[])
    }

    pub fn preprocess_with_defines(
        &self,
        file_id: FileId,
        source: &str,
        cli_defines: &[(String, String)],
    ) -> Partial<PreprocessedSource> {
        let mut masked_source = String::with_capacity(source.len());
        let mut defines = Defines::default();
        let mut conditionals = Conditionals::default();
        let mut emitter = DiagnosticEmitter::new();

        emitter.extend(seed_cli_defines(file_id, cli_defines, &mut defines));

        iterate_lines(source, |line_start, line, newline| {
            let active_code_end = comment_start(line).unwrap_or(line.len());
            let active_code = &line[..active_code_end];
            let trailing_comment = &line[active_code_end..];
            let is_active = current_active(&conditionals);

            match parse_preprocessor_directive(file_id, line_start, active_code) {
                Ok(Some(directive)) => {
                    emitter.extend(handle_directive(
                        directive,
                        file_id,
                        &mut defines,
                        &mut conditionals,
                        is_active,
                    ));
                    masked_source.push_str(&mask_text(line));
                    masked_source.push_str(newline);
                    return;
                }
                Ok(None) => {}
                Err(diagnostic) => {
                    emitter.push(diagnostic);
                    masked_source.push_str(&mask_text(line));
                    masked_source.push_str(newline);
                    return;
                }
            }

            if is_active {
                masked_source.push_str(active_code);
                masked_source.push_str(&mask_text(trailing_comment));
            } else {
                masked_source.push_str(&mask_text(line));
            }
            masked_source.push_str(newline);
        });

        let eof_span = Span::new(file_id, masked_source.len(), masked_source.len());
        emitter.extend(unterminated_conditional_diagnostics(
            &conditionals,
            eof_span,
        ));

        let tokenized = Tokenizer::new(file_id, &masked_source).tokenize();
        emitter.extend(tokenized.diagnostics);
        let Some(tokens) = tokenized.value else {
            return emitter.fail();
        };

        let expanded = expand_defines(&tokens, &defines);
        emitter.extend(expanded.diagnostics);
        let Some(tokens) = expanded.value else {
            return emitter.fail();
        };

        emitter.finish(PreprocessedSource {
            masked_source,
            tokens,
        })
    }
}

fn seed_cli_defines(
    file_id: FileId,
    cli_defines: &[(String, String)],
    defines: &mut Defines,
) -> Vec<crate::diagnostics::Diagnostic> {
    let mut emitter = DiagnosticEmitter::new();

    for (name, replacement) in cli_defines {
        let replacement_tokens = Tokenizer::new(file_id, replacement).tokenize();
        emitter.extend(replacement_tokens.diagnostics);
        let Some(replacement_tokens) = replacement_tokens.value else {
            continue;
        };

        let replacement = replacement_tokens
            .into_iter()
            .filter(|token| {
                !matches!(
                    token.kind,
                    crate::frontend::syntax::tokens::TokenKind::Eof
                        | crate::frontend::syntax::tokens::TokenKind::Newline
                )
            })
            .map(|token| token.kind)
            .collect();

        defines.insert(
            name.clone(),
            crate::preprocessing::context::Define {
                replacement,
                span: Span::new(file_id, 0, 0),
            },
        );
    }

    emitter.into_diagnostics()
}

#[cfg(test)]
mod tests {
    use crate::{
        frontend::syntax::{parser::Parser, statements::Statement},
        preprocessing::Preprocessor,
    };

    #[test]
    fn masks_inactive_branches_before_parsing() {
        let source = ".ifdef OFF\nwat r0\n.else\nhalt\n.endif\n";
        let processed = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap();
        let program = Parser::new(&processed.tokens)
            .parse()
            .into_result()
            .unwrap();

        assert!(matches!(
            &program.statements[0],
            Statement::Instruction(instruction)
                if instruction.mnemonic == "halt" && instruction.operands.is_empty()
        ));
    }

    #[test]
    fn rejects_unterminated_conditionals() {
        let source = ".ifdef FOO\nhalt\n";
        let errors = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap_err();

        assert_eq!(errors[0].message, "unterminated conditional directive");
    }
}
