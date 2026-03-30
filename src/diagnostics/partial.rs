use super::Diagnostic;

#[derive(Debug, Clone)]
pub struct Partial<T> {
    pub value: Option<T>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> Partial<T> {
    pub fn success(value: T) -> Self {
        Self {
            value: Some(value),
            diagnostics: Vec::new(),
        }
    }

    pub fn with_diagnostics(value: T, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            value: Some(value),
            diagnostics,
        }
    }

    pub fn failure(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            value: None,
            diagnostics,
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Partial<U> {
        Partial {
            value: self.value.map(f),
            diagnostics: self.diagnostics,
        }
    }

    pub fn into_result(self) -> Result<T, Vec<Diagnostic>> {
        if self.diagnostics.is_empty() {
            self.value.ok_or_else(Vec::new)
        } else {
            Err(self.diagnostics)
        }
    }
}
