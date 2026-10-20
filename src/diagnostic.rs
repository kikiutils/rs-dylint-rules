use ra_ap_syntax::TextRange;
use rustc_errors::{
    Applicability,
    DiagDecorator,
};
use rustc_hir::CRATE_HIR_ID;
use rustc_lint::LateContext;

use crate::{
    rules::Rule,
    source::Source,
};

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

    // Diagnostics are emitted at crate level; item and module lint levels are not consulted.
    pub(crate) fn emit(&self, cx: &LateContext<'_>, rule: Rule, source: &Source, can_fix: bool) {
        for issue in &self.issues {
            let Some((diagnostic, others)) = issue.diagnostics.split_first() else {
                continue;
            };

            cx.tcx.emit_node_span_lint(
                rule.lint(),
                CRATE_HIR_ID,
                source.span(diagnostic.range),
                DiagDecorator(|diag| {
                    diag.primary_message(diagnostic.message.clone());
                    for other in others {
                        diag.span_label(source.span(other.range), other.message.clone());
                    }

                    if let Some(fix) = issue.fix.as_ref().filter(|_| can_fix) {
                        diag.multipart_suggestion(
                            rule.help(),
                            fix.edits
                                .iter()
                                .map(|edit| (source.span(edit.range), edit.replacement.clone()))
                                .collect(),
                            Applicability::MachineApplicable,
                        );
                    }
                }),
            );
        }
    }
}
