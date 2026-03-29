mod codes;
mod diagnostic;
mod emitter;

pub use codes::DiagnosticCode;
pub use diagnostic::{Diagnostic, DiagnosticLabel, FileId, Severity, Span};
pub use emitter::DiagnosticEmitter;
