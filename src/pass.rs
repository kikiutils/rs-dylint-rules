use rustc_hir as hir;
use rustc_lint::{
    LateContext,
    LateLintPass,
};
use rustc_lint_defs::{
    LintPass,
    LintVec,
};
use rustc_span::def_id::LocalModId;

use crate::{
    rules::{
        self,
        Collectors,
        RULES,
    },
    source::Source,
    workspace::Workspace,
};

#[derive(Default)]
pub(crate) struct KikiRules {
    workspace: Workspace,
    collectors: Collectors,
}

impl LintPass for KikiRules {
    fn name(&self) -> &'static str {
        "KikiRules"
    }

    fn get_lints(&self) -> LintVec {
        rules::lints()
    }
}

impl<'tcx> LateLintPass<'tcx> for KikiRules {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx hir::Mod<'tcx>, id: LocalModId) {
        self.workspace.module(cx, module.spans.inner_span, id);
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        self.collectors.check_item(cx, &mut self.workspace, item);
    }

    fn check_foreign_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ForeignItem<'tcx>) {
        self.collectors.check_foreign_item(cx, item);
    }

    fn check_generic_param(&mut self, cx: &LateContext<'tcx>, param: &'tcx hir::GenericParam<'tcx>) {
        self.collectors.check_generic_param(cx, param);
    }

    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &hir::Path<'tcx>, id: hir::HirId) {
        self.collectors.check_path(cx, &mut self.workspace, path, id);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        self.collectors.check_expr(cx, &mut self.workspace, expr);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // Each path is visited once, so it is read, parsed and warned about at most once.
        for (path, anchor) in &self.workspace.files {
            if !self.workspace.modules.contains_path(path) {
                continue;
            }

            let Some(source) = Source::load(cx, path, *anchor) else {
                continue;
            };

            let can_fix = self.workspace.modules.can_fix(path);
            for rule in RULES {
                rule.analyze(&self.collectors, cx, path, &source, &self.workspace)
                    .emit(cx, *rule, &source, can_fix);
            }
        }
    }
}
