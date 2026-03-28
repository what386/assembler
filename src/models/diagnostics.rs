pub type FileId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub file_id: FileId,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone)]
pub struct DiagnosticLabel {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<DiagnosticLabel>,
}
