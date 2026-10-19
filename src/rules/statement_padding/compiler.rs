use std::collections::BTreeMap;

use pathkit::Path;
use ra_ap_syntax::{
    AstNode,
    TextRange,
};
use rustc_hir::{
    self as hir,
    def_id::LocalModId,
};
use rustc_lint::{
    LateContext,
    LateLintPass,
};
use rustc_lint_defs::{
    declare_lint,
    impl_lint_pass,
};
use rustc_span::Span;

use super::analysis;

declare_lint! { pub KIKI_STATEMENT_PADDING, Warn, "separate multi-line statements and adjacent items with blank lines" }

pub(super) fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[KIKI_STATEMENT_PADDING]);
    store.register_late_lint_pass(Box::new(|_| Box::<StatementPadding>::default()));
}

#[derive(Default)]
struct StatementPadding {
    files: BTreeMap<Path, FileFacts>,
    sources: crate::source::ModuleSources,
}

struct FileFacts {
    anchor: Span,
    // Owners of lint levels, used to honor attributes on the enclosing item.
    items: Vec<(TextRange, hir::HirId)>,
}

impl_lint_pass!(StatementPadding => [KIKI_STATEMENT_PADDING]);

impl StatementPadding {
    fn record(&mut self, cx: &LateContext<'_>, span: Span, id: hir::HirId) {
        let Some((path, range)) = crate::source::locate(cx, span) else {
            return;
        };

        self.files
            .entry(path)
            .or_insert_with(|| FileFacts {
                anchor: span,
                items: Vec::new(),
            })
            .items
            .push((range, id));
    }
}

impl<'tcx> LateLintPass<'tcx> for StatementPadding {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx hir::Mod<'tcx>, id: LocalModId) {
        self.sources.record(
            cx,
            module.spans.inner_span,
            id.to_local_def_id().local_def_index.as_u32(),
        );
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        self.record(cx, item.span, item.hir_id());
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        self.record(cx, item.span, item.hir_id());
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        self.record(cx, item.span, item.hir_id());
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (path, facts) in &self.files {
            if !self.sources.contains_path(path) {
                continue;
            }

            let Some(source) = crate::source::Source::load(cx, path, facts.anchor) else {
                continue;
            };

            let output = analysis::analyze(source.parsed.syntax());
            for issue in output.issues {
                let diagnostic = &issue.diagnostics[0];
                // The innermost owner of the element's last character decides the lint level.
                let id = facts
                    .items
                    .iter()
                    .filter(|(range, _)| range.contains_inclusive(diagnostic.range.end()))
                    .min_by_key(|(range, _)| range.len())
                    .map_or(hir::CRATE_HIR_ID, |(_, id)| *id);

                issue.emit(
                    cx,
                    KIKI_STATEMENT_PADDING,
                    id,
                    &source,
                    "add a blank line",
                    self.sources.can_fix(path),
                );
            }
        }
    }
}
