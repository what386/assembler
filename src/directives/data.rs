use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, DiagnosticLabel, Span},
    frontend::syntax::statements::{DirectiveArg, DirectiveStatement},
};

pub fn validate_data_directive(directive: &DirectiveStatement) -> Option<Result<(), Diagnostic>> {
    match directive.name.as_str() {
        "string" | "cstring" => Some(validate_string_like(directive)),
        "fill" => Some(validate_fill(directive)),
        "bytes" => Some(validate_bytes(directive)),
        _ => None,
    }
}

pub fn directive_data_len(directive: &DirectiveStatement) -> Option<Result<i64, Diagnostic>> {
    match directive.name.as_str() {
        "bytes" => Some(Ok(directive.args.len() as i64)),
        "string" => Some(match directive.args.first() {
            Some(DirectiveArg::String(value)) => Ok(value.len() as i64),
            _ => Err(directive_error(directive, "expected string argument")),
        }),
        "cstring" => Some(match directive.args.first() {
            Some(DirectiveArg::String(value)) => Ok(value.len() as i64 + 1),
            _ => Err(directive_error(directive, "expected string argument")),
        }),
        "fill" => Some(expect_integer_arg(directive, 0)),
        _ => None,
    }
}

pub fn encode_data_directive(
    directive: &DirectiveStatement,
    image: &mut Vec<u8>,
    cursor: &mut usize,
    emitter: &mut DiagnosticEmitter,
) -> bool {
    match directive.name.as_str() {
        "bytes" => {
            for arg in &directive.args {
                let Some(value) = literal_value(arg) else {
                    emitter.push(encoding_error(
                        directive.span,
                        "directive `.bytes` expects integer or char arguments",
                    ));
                    continue;
                };

                let Ok(byte) = encode_unsigned_value(
                    value,
                    8,
                    directive.span,
                    "byte literal does not fit in 8 bits",
                ) else {
                    emitter.push(encoding_error(
                        directive.span,
                        "byte literal does not fit in 8 bits",
                    ));
                    continue;
                };
                write_byte(image, *cursor, byte as u8);
                *cursor += 1;
            }
            true
        }
        "fill" => {
            let Some(count) = directive_int(directive, 0, emitter) else {
                return true;
            };
            if count < 0 {
                emitter.push(encoding_error(
                    directive.span,
                    "directive `.fill` expects a non-negative count",
                ));
                return true;
            }

            let Some(value) = directive.args.get(1).and_then(literal_value) else {
                emitter.push(encoding_error(
                    directive.span,
                    "directive `.fill` expects an integer or char fill value",
                ));
                return true;
            };

            let Ok(byte) = encode_unsigned_value(
                value,
                8,
                directive.span,
                "fill value does not fit in 8 bits",
            ) else {
                emitter.push(encoding_error(
                    directive.span,
                    "fill value does not fit in 8 bits",
                ));
                return true;
            };

            for _ in 0..count as usize {
                write_byte(image, *cursor, byte as u8);
                *cursor += 1;
            }
            true
        }
        "string" => {
            match directive.args.first() {
                Some(DirectiveArg::String(value)) => {
                    for byte in value.bytes() {
                        write_byte(image, *cursor, byte);
                        *cursor += 1;
                    }
                }
                _ => emitter.push(encoding_error(
                    directive.span,
                    "directive `.string` expects a string argument",
                )),
            }
            true
        }
        "cstring" => {
            match directive.args.first() {
                Some(DirectiveArg::String(value)) => {
                    for byte in value.bytes() {
                        write_byte(image, *cursor, byte);
                        *cursor += 1;
                    }
                    write_byte(image, *cursor, 0);
                    *cursor += 1;
                }
                _ => emitter.push(encoding_error(
                    directive.span,
                    "directive `.cstring` expects a string argument",
                )),
            }
            true
        }
        _ => false,
    }
}

fn validate_string_like(directive: &DirectiveStatement) -> Result<(), Diagnostic> {
    if matches!(directive.args.first(), Some(DirectiveArg::String(_))) {
        Ok(())
    } else {
        Err(directive_error(directive, "expected string argument"))
    }
}

fn validate_fill(directive: &DirectiveStatement) -> Result<(), Diagnostic> {
    if directive.args.len() != 2 {
        return Err(directive_error(directive, "expected count and fill value"));
    }

    if !matches!(
        directive.args.first(),
        Some(DirectiveArg::Integer { .. } | DirectiveArg::Char { .. })
    ) {
        return Err(directive_error(directive, "expected integer count"));
    }

    if matches!(
        directive.args.get(1),
        Some(DirectiveArg::Integer { .. } | DirectiveArg::Char { .. })
    ) {
        Ok(())
    } else {
        Err(directive_error(
            directive,
            "expected integer or char fill value",
        ))
    }
}

fn validate_bytes(directive: &DirectiveStatement) -> Result<(), Diagnostic> {
    if directive.args.iter().all(|arg| {
        matches!(
            arg,
            DirectiveArg::Integer { .. } | DirectiveArg::Char { .. }
        )
    }) {
        Ok(())
    } else {
        Err(directive_error(directive, "expected only byte-sized literals"))
    }
}

fn expect_integer_arg(directive: &DirectiveStatement, index: usize) -> Result<i64, Diagnostic> {
    match directive.args.get(index) {
        Some(DirectiveArg::Integer { value, .. }) => Ok(*value),
        Some(DirectiveArg::Char { value, .. }) => Ok(*value),
        _ => Err(directive_error(directive, "expected integer argument")),
    }
}

fn directive_error(directive: &DirectiveStatement, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    Diagnostic::error_code(DiagnosticCode::InvalidDirective(message.clone()))
        .with_label(DiagnosticLabel::new(directive.span, message))
}

fn literal_value(arg: &DirectiveArg) -> Option<i64> {
    match arg {
        DirectiveArg::Integer { value, .. } | DirectiveArg::Char { value, .. } => Some(*value),
        DirectiveArg::Identifier(_) | DirectiveArg::String(_) => None,
    }
}

fn directive_int(
    directive: &DirectiveStatement,
    index: usize,
    emitter: &mut DiagnosticEmitter,
) -> Option<i64> {
    match directive.args.get(index).and_then(literal_value) {
        Some(value) => Some(value),
        None => {
            emitter.push(encoding_error(
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

fn encode_unsigned_value(
    value: i64,
    bit_length: u8,
    span: Span,
    message: &str,
) -> Result<u32, Diagnostic> {
    if value < 0 {
        return Err(encoding_error(span, message));
    }

    let max = if bit_length == 64 {
        i64::MAX
    } else {
        (1_i64 << bit_length) - 1
    };
    if value > max {
        return Err(encoding_error(span, message));
    }

    Ok(value as u32)
}

fn encoding_error(span: Span, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    Diagnostic::error_code(DiagnosticCode::EncodingError(message.clone()))
        .with_label(DiagnosticLabel::new(span, message))
}

fn write_byte(image: &mut Vec<u8>, at: usize, byte: u8) {
    if image.len() < at + 1 {
        image.resize(at + 1, 0);
    }
    image[at] = byte;
}
