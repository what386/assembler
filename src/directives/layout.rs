use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, DiagnosticLabel},
    frontend::syntax::statements::{DirectiveArg, DirectiveStatement},
};

pub fn validate_layout_directive(
    directive: &DirectiveStatement,
) -> Option<Result<(), Diagnostic>> {
    match directive.name.as_str() {
        "page" | "org" => Some(validate_integer_arg(directive)),
        _ => None,
    }
}

pub fn apply_layout_directive(
    directive: &DirectiveStatement,
    cursor: i64,
    current_page_start: &mut Option<i64>,
    emitter: &mut DiagnosticEmitter,
) -> Option<i64> {
    match directive.name.as_str() {
        "page" => {
            let Some(page) = directive_int(directive, 0, emitter) else {
                return Some(cursor);
            };
            if page < 0 {
                emitter.push(page_error(
                    directive.span,
                    "directive `.page` expects a non-negative page number",
                ));
                return Some(cursor);
            }

            let next = page << 7;
            *current_page_start = Some(next);
            Some(next)
        }
        "org" => {
            let Some(target) = directive_int(directive, 0, emitter) else {
                return Some(cursor);
            };
            if target < 0 {
                emitter.push(page_error(
                    directive.span,
                    "directive `.org` expects a non-negative address",
                ));
                return Some(cursor);
            }

            *current_page_start = None;
            Some(target)
        }
        _ => None,
    }
}

fn validate_integer_arg(directive: &DirectiveStatement) -> Result<(), Diagnostic> {
    if matches!(
        directive.args.first(),
        Some(DirectiveArg::Integer { .. } | DirectiveArg::Char { .. })
    ) {
        Ok(())
    } else {
        Err(directive_error(directive, "expected integer argument"))
    }
}

pub fn directive_int(
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

pub fn page_error(
    span: crate::diagnostics::Span,
    message: impl Into<String>,
) -> Diagnostic {
    let message = message.into();
    Diagnostic::error_code(DiagnosticCode::EncodingError(message.clone()))
        .with_label(DiagnosticLabel::new(span, message))
}

fn directive_error(directive: &DirectiveStatement, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    Diagnostic::error_code(DiagnosticCode::InvalidDirective(message.clone()))
        .with_label(DiagnosticLabel::new(directive.span, message))
}
