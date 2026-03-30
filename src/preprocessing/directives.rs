use crate::{
    diagnostics::{Diagnostic, DiagnosticEmitter, DiagnosticLabel, FileId, Partial, Span},
    frontend::syntax::{
        lexer::Tokenizer,
        tokens::{Token, TokenKind},
    },
};

use crate::preprocessing::{
    context::{
        ConditionalFrame, ConditionalKind, Conditionals, Define, Defines, diagnostics_from,
        directive_error,
    },
    source_file::shift_diagnostic,
};

#[derive(Debug, Clone)]
pub enum PreprocessorDirective<'a> {
    Define {
        directive_span: Span,
        name: &'a str,
        name_span: Span,
        replacement: &'a str,
        replacement_offset: usize,
    },
    Conditional {
        directive_span: Span,
        kind: ConditionalKind,
        name: &'a str,
    },
    Else {
        directive_span: Span,
    },
    Endif {
        directive_span: Span,
    },
}

pub fn handle_directive(
    directive: PreprocessorDirective<'_>,
    file_id: FileId,
    defines: &mut Defines,
    conditionals: &mut Conditionals,
    current_active: bool,
) -> Vec<Diagnostic> {
    let mut emitter = DiagnosticEmitter::new();

    match directive {
        PreprocessorDirective::Define {
            directive_span,
            name,
            name_span,
            replacement,
            replacement_offset,
        } => {
            if !current_active {
                return diagnostics_from(emitter);
            }

            if let Some(previous) = defines.get(name) {
                emitter.push(
                    Diagnostic::error_code(crate::diagnostics::DiagnosticCode::InvalidDirective(
                        format!("define `{name}` is already set"),
                    ))
                    .with_label(DiagnosticLabel::new(
                        name_span,
                        format!("`{name}` redefined here"),
                    ))
                    .with_label(DiagnosticLabel::secondary(
                        previous.span,
                        "previous definition here",
                    )),
                );
                return diagnostics_from(emitter);
            }

            let replacement_tokens =
                tokenize_define_replacement(file_id, replacement, replacement_offset);
            if replacement_tokens.has_errors() {
                emitter.extend(replacement_tokens.diagnostics);
                return diagnostics_from(emitter);
            }

            let replacement = replacement_tokens
                .value
                .unwrap_or_default()
                .into_iter()
                .map(|token| token.kind)
                .collect::<Vec<_>>();

            defines.insert(
                name.to_owned(),
                Define {
                    replacement,
                    span: directive_span,
                },
            );
        }
        PreprocessorDirective::Conditional {
            directive_span,
            kind,
            name,
        } => {
            let condition_true = match kind {
                ConditionalKind::Ifdef => defines.contains_key(name),
                ConditionalKind::Ifndef => !defines.contains_key(name),
            };
            conditionals.push(ConditionalFrame {
                start_span: directive_span,
                parent_active: current_active,
                condition_true,
                saw_else: false,
            });
        }
        PreprocessorDirective::Else { directive_span } => {
            let Some(frame) = conditionals.last_mut() else {
                emitter.push(directive_error(
                    directive_span,
                    "directive `.else` has no matching conditional",
                ));
                return diagnostics_from(emitter);
            };

            if frame.saw_else {
                emitter.push(directive_error(
                    directive_span,
                    "directive `.else` may only appear once per conditional block",
                ));
                return diagnostics_from(emitter);
            }

            frame.saw_else = true;
        }
        PreprocessorDirective::Endif { directive_span } => {
            if conditionals.pop().is_none() {
                emitter.push(directive_error(
                    directive_span,
                    "directive `.endif` has no matching conditional",
                ));
            }
        }
    }

    diagnostics_from(emitter)
}

pub fn expand_defines(tokens: &[Token], defines: &Defines) -> Partial<Vec<Token>> {
    let mut expanded = Vec::with_capacity(tokens.len());
    let mut emitter = DiagnosticEmitter::new();

    for token in tokens {
        if matches!(token.kind, TokenKind::Eof) {
            expanded.push(token.clone());
            continue;
        }

        expand_token(token, defines, &mut expanded, &mut Vec::new(), &mut emitter);
    }

    emitter.finish(expanded)
}

fn tokenize_define_replacement(
    file_id: FileId,
    replacement: &str,
    replacement_offset: usize,
) -> Partial<Vec<Token>> {
    if replacement.trim().is_empty() {
        return Partial::failure(vec![directive_error(
            Span::new(file_id, replacement_offset, replacement_offset),
            "directive `.define` expected a replacement value",
        )]);
    }

    let mut tokenized = Tokenizer::new(file_id, replacement).tokenize();
    for diagnostic in &mut tokenized.diagnostics {
        *diagnostic = shift_diagnostic(diagnostic.clone(), replacement_offset);
    }

    tokenized.map(|tokens| {
        tokens
            .into_iter()
            .filter(|token| !matches!(token.kind, TokenKind::Eof | TokenKind::Newline))
            .collect()
    })
}

fn expand_token(
    token: &Token,
    defines: &Defines,
    out: &mut Vec<Token>,
    stack: &mut Vec<String>,
    emitter: &mut DiagnosticEmitter,
) {
    let TokenKind::Identifier(name) = &token.kind else {
        out.push(token.clone());
        return;
    };

    let Some(define) = defines.get(name) else {
        out.push(token.clone());
        return;
    };

    if stack.iter().any(|seen| seen == name) {
        emitter.push(
            Diagnostic::error_code(crate::diagnostics::DiagnosticCode::InvalidDirective(
                format!("recursive define expansion for `{name}`"),
            ))
            .with_label(DiagnosticLabel::new(
                token.span,
                format!("`{name}` expands recursively here"),
            ))
            .with_label(DiagnosticLabel::secondary(
                define.span,
                "definition involved in recursion",
            )),
        );
        out.push(token.clone());
        return;
    }

    stack.push(name.clone());
    for kind in &define.replacement {
        let generated = Token {
            kind: kind.clone(),
            span: token.span,
        };
        expand_token(&generated, defines, out, stack, emitter);
    }
    stack.pop();
}

pub fn parse_preprocessor_directive<'a>(
    file_id: FileId,
    line_start: usize,
    line: &'a str,
) -> Result<Option<PreprocessorDirective<'a>>, Diagnostic> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let leading = line.len() - trimmed.len();
    if !trimmed.starts_with('.') {
        return Ok(None);
    }

    let rest = &trimmed[1..];
    let Some((directive_name, directive_name_len)) = parse_identifier(rest) else {
        return Ok(None);
    };
    let directive_start = line_start + leading;
    let directive_span = Span::new(
        file_id,
        directive_start,
        directive_start + 1 + directive_name_len,
    );
    let args = &rest[directive_name_len..];

    match directive_name {
        "define" => {
            parse_define_directive(file_id, directive_span, args, line_start + line.len()).map(Some)
        }
        "ifdef" => {
            parse_conditional_directive(directive_span, args, ConditionalKind::Ifdef).map(Some)
        }
        "ifndef" => {
            parse_conditional_directive(directive_span, args, ConditionalKind::Ifndef).map(Some)
        }
        "else" => {
            ensure_no_extra_args(file_id, directive_span, args)?;
            Ok(Some(PreprocessorDirective::Else { directive_span }))
        }
        "endif" => {
            ensure_no_extra_args(file_id, directive_span, args)?;
            Ok(Some(PreprocessorDirective::Endif { directive_span }))
        }
        _ => Ok(None),
    }
}

fn parse_define_directive<'a>(
    file_id: FileId,
    directive_span: Span,
    args: &'a str,
    line_end: usize,
) -> Result<PreprocessorDirective<'a>, Diagnostic> {
    let args_trimmed = args.trim_start_matches([' ', '\t']);
    let skipped = args.len() - args_trimmed.len();
    let name_start = directive_span.end + skipped;
    let Some((name, name_len)) = parse_identifier(args_trimmed) else {
        return Err(directive_error(
            directive_span,
            "directive `.define` expected an identifier name",
        ));
    };

    let name_span = Span::new(file_id, name_start, name_start + name_len);
    let replacement = &args_trimmed[name_len..];
    let replacement_trimmed = replacement.trim_start_matches([' ', '\t']);
    let replacement_skipped = replacement.len() - replacement_trimmed.len();
    let replacement_offset = name_span.end + replacement_skipped;
    if replacement_trimmed.is_empty() {
        return Err(directive_error(
            Span::new(file_id, line_end, line_end),
            "directive `.define` expected a replacement value",
        ));
    }

    Ok(PreprocessorDirective::Define {
        directive_span,
        name,
        name_span,
        replacement: replacement_trimmed,
        replacement_offset,
    })
}

fn parse_conditional_directive<'a>(
    directive_span: Span,
    args: &'a str,
    kind: ConditionalKind,
) -> Result<PreprocessorDirective<'a>, Diagnostic> {
    let args_trimmed = args.trim_start_matches([' ', '\t']);
    let Some((name, name_len)) = parse_identifier(args_trimmed) else {
        return Err(directive_error(
            directive_span,
            match kind {
                ConditionalKind::Ifdef => "directive `.ifdef` expected an identifier name",
                ConditionalKind::Ifndef => "directive `.ifndef` expected an identifier name",
            },
        ));
    };

    let extra = args_trimmed[name_len..].trim();
    if !extra.is_empty() {
        return Err(directive_error(
            directive_span,
            "conditional directives accept only a single identifier argument",
        ));
    }

    Ok(PreprocessorDirective::Conditional {
        directive_span,
        kind,
        name,
    })
}

fn ensure_no_extra_args(
    file_id: FileId,
    directive_span: Span,
    args: &str,
) -> Result<(), Diagnostic> {
    if args.trim().is_empty() {
        Ok(())
    } else {
        Err(directive_error(
            Span::new(file_id, directive_span.end, directive_span.end + args.len()),
            "directive does not accept arguments",
        ))
    }
}

fn parse_identifier(input: &str) -> Option<(&str, usize)> {
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return None;
    }

    let mut end = first.len_utf8();
    for (index, ch) in chars {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    Some((&input[..end], end))
}

#[cfg(test)]
mod tests {
    use crate::{
        frontend::syntax::{lexer::Tokenizer, tokens::TokenKind},
        preprocessing::Preprocessor,
    };

    #[test]
    fn expands_defines_at_use_site_spans() {
        let source = ".define VALUE 42\nlim r0, VALUE\n";
        let processed = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap();
        let value_start = source.find("VALUE\n").unwrap();
        let expanded = processed
            .tokens
            .iter()
            .find(|token| matches!(token.kind, TokenKind::Integer { value: 42, .. }))
            .unwrap();

        assert_eq!(expanded.span.start, value_start);
    }

    #[test]
    fn rejects_define_redefinition() {
        let source = ".define VALUE 1\n.define VALUE 2\n";
        let errors = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap_err();

        assert_eq!(errors[0].message, "define `VALUE` is already set");
    }

    #[test]
    fn treats_define_names_as_case_sensitive() {
        let source = ".define VALUE 42\nlim r0, value\n";
        let processed = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap();

        assert!(processed.tokens.iter().any(|token| matches!(
            token.kind,
            TokenKind::Identifier(ref name) if name == "value"
        )));
        assert!(
            !processed
                .tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Integer { value: 42, .. }))
        );
    }

    #[test]
    fn rejects_recursive_defines() {
        let source = ".define A B\n.define B A\nlim r0, A\n";
        let errors = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap_err();

        assert_eq!(errors[0].message, "recursive define expansion for `A`");
    }

    #[test]
    fn collects_multiple_directive_errors() {
        let source = ".else\n.endif extra\n.ifdef too many args\n";
        let errors = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap_err();

        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn tokenization_of_replacements_matches_lexer() {
        let tokens = Tokenizer::new(0, "1 + 2").tokenize().into_result().unwrap();
        assert!(tokens.len() > 1);
    }
}
