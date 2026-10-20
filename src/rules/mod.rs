use pathkit::Path;
use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_lint_defs::Lint;

mod grouped_derive_order;
mod import_module_order;
mod import_path_prefix;
mod prefer_imported_apis;
mod statement_padding;

use crate::{
    diagnostic::RuleOutput,
    source::Source,
    workspace::Workspace,
};

// Every rule, for lint registration and per-file analysis. The order only sets
// diagnostic order and which overlapping fix applies first; `cargo fix` reruns
// until stable, so it never changes the final result.
pub(crate) const RULES: &[Rule] = &[
    Rule::PreferImportedApis,
    Rule::ImportPathPrefix,
    Rule::ImportModuleOrder,
    Rule::GroupedDeriveOrder,
    Rule::StatementPadding,
];

#[derive(Clone, Copy)]
pub(crate) enum Rule {
    PreferImportedApis,
    ImportPathPrefix,
    ImportModuleOrder,
    GroupedDeriveOrder,
    StatementPadding,
}

#[derive(Default)]
pub(crate) struct Collectors {
    prefer_imported_apis: prefer_imported_apis::collect::Collector,
    import_path_prefix: import_path_prefix::collect::Collector,
    import_module_order: import_module_order::collect::Collector,
    grouped_derive_order: grouped_derive_order::collect::Collector,
}

// HIR hooks forwarded from the lint pass to the collectors that need them.
impl Collectors {
    pub(crate) fn check_item<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        item: &'tcx hir::Item<'tcx>,
    ) {
        self.prefer_imported_apis.check_item(cx, workspace, item);
        self.import_path_prefix.check_item(cx, workspace, item);
        self.import_module_order.check_item(cx, workspace, item);
        self.grouped_derive_order.check_item(cx, workspace, item);
    }

    pub(crate) fn check_foreign_item<'tcx>(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ForeignItem<'tcx>) {
        self.prefer_imported_apis.check_foreign_item(cx, item);
    }

    pub(crate) fn check_generic_param<'tcx>(&mut self, cx: &LateContext<'tcx>, param: &'tcx hir::GenericParam<'tcx>) {
        self.prefer_imported_apis.check_generic_param(cx, param);
    }

    pub(crate) fn check_path<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        path: &hir::Path<'tcx>,
        id: hir::HirId,
    ) {
        self.prefer_imported_apis.check_path(cx, workspace, path, id);
    }

    pub(crate) fn check_expr<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        expr: &'tcx hir::Expr<'tcx>,
    ) {
        self.prefer_imported_apis.check_expr(cx, workspace, expr);
    }
}

impl Rule {
    pub(crate) fn lint(self) -> &'static Lint {
        match self {
            Self::PreferImportedApis => prefer_imported_apis::KIKI_PREFER_IMPORTED_APIS,
            Self::ImportPathPrefix => import_path_prefix::KIKI_IMPORT_PATH_PREFIX,
            Self::ImportModuleOrder => import_module_order::KIKI_IMPORT_MODULE_ORDER,
            Self::GroupedDeriveOrder => grouped_derive_order::KIKI_GROUPED_DERIVE_ORDER,
            Self::StatementPadding => statement_padding::KIKI_STATEMENT_PADDING,
        }
    }

    pub(crate) fn help(self) -> &'static str {
        match self {
            Self::PreferImportedApis => "import the API and use its prescribed name",
            Self::ImportPathPrefix => "use the module-relative import prefix",
            Self::ImportModuleOrder => "reorder import/module sections",
            Self::GroupedDeriveOrder => "reorder this derive list",
            Self::StatementPadding => "add a blank line",
        }
    }

    pub(crate) fn analyze(
        self,
        collectors: &Collectors,
        cx: &LateContext<'_>,
        path: &Path,
        source: &Source,
        workspace: &Workspace,
    ) -> RuleOutput {
        match self {
            Self::PreferImportedApis => collectors.prefer_imported_apis.analyze(cx, path, source, workspace),
            Self::ImportPathPrefix => collectors.import_path_prefix.analyze(path, source, workspace),
            Self::ImportModuleOrder => collectors.import_module_order.analyze(path, source, workspace),
            Self::GroupedDeriveOrder => collectors.grouped_derive_order.analyze(path, source, workspace),
            Self::StatementPadding => statement_padding::analyze(source),
        }
    }
}

pub(crate) fn lints() -> rustc_lint_defs::LintVec {
    RULES.iter().map(|rule| rule.lint()).collect()
}
