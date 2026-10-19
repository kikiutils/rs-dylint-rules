use ra_ap_syntax::TextRange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub(crate) range: TextRange,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Edit {
    pub(crate) range: TextRange,
    pub(crate) replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fix {
    pub(crate) edits: Vec<Edit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Issue {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) fix: Option<Fix>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct RuleOutput {
    pub(crate) issues: Vec<Issue>,
}

impl RuleOutput {
    pub(crate) fn report(&mut self, diagnostic: Diagnostic) {
        self.issues.push(Issue {
            diagnostics: vec![diagnostic],
            fix: None,
        });
    }
}
