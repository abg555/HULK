use crate::ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Span,
    pub hints: Vec<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span,
            hints: Vec::new(),
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            span,
            hints: Vec::new(),
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hints.push(hint.into());
        self
    }
}

#[derive(Debug, Default)]
pub struct DiagnosticCollector {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        if self
            .diagnostics
            .iter()
            .any(|existing| existing == &diagnostic)
        {
            return;
        }

        self.diagnostics.push(diagnostic);
    }

    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::error(message, span));
    }

    pub fn warning(&mut self, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::warning(message, span));
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
    }

    pub fn to_vec(&self) -> Vec<Diagnostic> {
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.sort_by(|a, b| {
            a.span
                .start
                .cmp(&b.span.start)
                .then_with(|| a.span.end.cmp(&b.span.end))
                .then_with(|| a.message.cmp(&b.message))
        });
        diagnostics
    }

    pub fn into_vec(mut self) -> Vec<Diagnostic> {
        self.diagnostics.sort_by(|a, b| {
            a.span
                .start
                .cmp(&b.span.start)
                .then_with(|| a.span.end.cmp(&b.span.end))
                .then_with(|| a.message.cmp(&b.message))
        });
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_identical_diagnostics() {
        let span = Span { start: 10, end: 20 };
        let mut collector = DiagnosticCollector::new();

        collector.push(Diagnostic::error("mismatch", span));
        collector.push(Diagnostic::error("mismatch", span));

        let diagnostics = collector.into_vec();
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn sorts_diagnostics_by_span_then_message() {
        let mut collector = DiagnosticCollector::new();

        collector.push(Diagnostic::error("z-msg", Span { start: 5, end: 8 }));
        collector.push(Diagnostic::error("a-msg", Span { start: 5, end: 8 }));
        collector.push(Diagnostic::error("mid", Span { start: 2, end: 3 }));

        let diagnostics = collector.into_vec();
        assert_eq!(diagnostics[0].message, "mid");
        assert_eq!(diagnostics[1].message, "a-msg");
        assert_eq!(diagnostics[2].message, "z-msg");
    }
}
