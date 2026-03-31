use std::path::{Path, PathBuf};

use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticLabel},
    frontend::syntax::statements::{DirectiveArg, DirectiveStatement},
};

#[derive(Debug, Clone, Default)]
pub struct IncbinContext {
    base_dir: Option<PathBuf>,
}

impl IncbinContext {
    pub fn new(base_dir: Option<PathBuf>) -> Self {
        Self { base_dir }
    }

    pub fn from_input_path(input_path: Option<&str>) -> Self {
        let base_dir = input_path.and_then(|input_path| {
            let path = Path::new(input_path);
            path.parent().map(Path::to_path_buf)
        });
        Self { base_dir }
    }
}

pub fn validate_incbin(directive: &DirectiveStatement) -> Result<(), Diagnostic> {
    if matches!(directive.args.first(), Some(DirectiveArg::String(_))) && directive.args.len() == 1
    {
        Ok(())
    } else {
        Err(directive_error(directive, "expected string path argument"))
    }
}

pub fn incbin_bytes(
    directive: &DirectiveStatement,
    context: &IncbinContext,
) -> Result<Vec<u8>, Diagnostic> {
    let path = incbin_path(directive, context)?;
    std::fs::read(&path).map_err(|error| {
        directive_error(
            directive,
            format!("failed to read `.incbin` file `{}`: {error}", path.display()),
        )
    })
}

pub fn incbin_length(
    directive: &DirectiveStatement,
    context: &IncbinContext,
) -> Result<i64, Diagnostic> {
    let path = incbin_path(directive, context)?;
    let metadata = std::fs::metadata(&path).map_err(|error| {
        directive_error(
            directive,
            format!(
                "failed to inspect `.incbin` file `{}`: {error}",
                path.display()
            ),
        )
    })?;
    i64::try_from(metadata.len()).map_err(|_| {
        directive_error(
            directive,
            format!("`.incbin` file `{}` is too large", path.display()),
        )
    })
}

fn incbin_path(
    directive: &DirectiveStatement,
    context: &IncbinContext,
) -> Result<PathBuf, Diagnostic> {
    let path = match directive.args.first() {
        Some(DirectiveArg::String(path)) => PathBuf::from(path),
        _ => {
            return Err(directive_error(
                directive,
                "directive `.incbin` expects a string path argument",
            ))
        }
    };

    if path.is_absolute() {
        return Ok(path);
    }

    let Some(base_dir) = &context.base_dir else {
        return Err(directive_error(
            directive,
            "directive `.incbin` requires an absolute path when reading source from stdin",
        ));
    };

    Ok(base_dir.join(path))
}

fn directive_error(directive: &DirectiveStatement, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    Diagnostic::error_code(DiagnosticCode::InvalidDirective(message.clone()))
        .with_label(DiagnosticLabel::new(directive.span, message))
}
