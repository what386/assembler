use std::collections::HashMap;

use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, DiagnosticLabel, Span},
    frontend::syntax::tokens::TokenKind,
};

#[derive(Debug, Clone)]
pub struct Define {
    pub replacement: Vec<TokenKind>,
    pub span: Span,
}

pub type Defines = HashMap<String, Define>;

#[derive(Debug, Clone, Copy)]
pub struct ConditionalFrame {
    pub start_span: Span,
    pub parent_active: bool,
    pub condition_true: bool,
    pub saw_else: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ConditionalKind {
    Ifdef,
    Ifndef,
}

pub type Conditionals = Vec<ConditionalFrame>;

impl ConditionalFrame {
    pub fn is_active(self) -> bool {
        self.parent_active
            && if self.saw_else {
                !self.condition_true
            } else {
                self.condition_true
            }
    }
}

pub fn current_active(conditionals: &[ConditionalFrame]) -> bool {
    conditionals
        .last()
        .copied()
        .is_none_or(ConditionalFrame::is_active)
}

pub fn directive_error(span: Span, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    Diagnostic::error_code(DiagnosticCode::InvalidDirective(message.clone()))
        .with_label(DiagnosticLabel::new(span, message))
}

pub fn unterminated_conditional_diagnostics(
    conditionals: &[ConditionalFrame],
    eof_span: Span,
) -> Vec<Diagnostic> {
    conditionals
        .iter()
        .map(|frame| {
            Diagnostic::error_code(DiagnosticCode::InvalidDirective(
                "unterminated conditional directive".to_owned(),
            ))
            .with_label(DiagnosticLabel::new(
                eof_span,
                if frame.saw_else {
                    "expected `.endif` after `.else`"
                } else {
                    "expected `.endif` before end of file"
                },
            ))
            .with_label(DiagnosticLabel::secondary(
                frame.start_span,
                "conditional starts here",
            ))
        })
        .collect()
}

pub fn diagnostics_from(emitter: DiagnosticEmitter) -> Vec<Diagnostic> {
    emitter.into_diagnostics()
}
