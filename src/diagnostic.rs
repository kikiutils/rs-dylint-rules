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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

impl Issue {
    pub(crate) fn emit(
        &self,
        cx: &rustc_lint::LateContext<'_>,
        lint: &'static rustc_lint_defs::Lint,
        id: rustc_hir::HirId,
        source: &crate::source::Source,
        help: &'static str,
        can_fix: bool,
    ) {
        let diagnostic = &self.diagnostics[0];
        cx.tcx.emit_node_span_lint(
            lint,
            id,
            source.span(diagnostic.range),
            rustc_errors::DiagDecorator(|diag| {
                diag.primary_message(diagnostic.message.clone());
                for other in self.diagnostics.iter().skip(1) {
                    diag.span_label(source.span(other.range), other.message.clone());
                }
                if let Some(fix) = self.fix.as_ref().filter(|_| can_fix) {
                    diag.multipart_suggestion(
                        help,
                        fix.edits
                            .iter()
                            .map(|edit| (source.span(edit.range), edit.replacement.clone()))
                            .collect(),
                        rustc_errors::Applicability::MachineApplicable,
                    );
                }
            }),
        );
    }
}
