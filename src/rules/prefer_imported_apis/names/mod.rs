use kikiutils::types::fx_collections::FxHashSet;
use ra_ap_syntax::TextRange;
use rustc_hir::{
    self as hir,
    def::Namespace,
};
use rustc_lint::LateContext;
use rustc_span::{
    Span,
    def_id::DefId,
};

mod bindings;
mod safety;

pub(super) struct NameRequest<'a> {
    pub(super) at: TextRange,
    pub(super) name: &'a str,
    pub(super) definition: DefId,
    pub(super) namespace: Namespace,
    pub(super) target: TextRange,
    pub(super) module_target: bool,
    pub(super) reuse: bool,
    pub(super) external: bool,
}

#[derive(Default)]
pub(super) struct Names {
    bindings: Vec<Name>,
    references: Vec<(Span, Name)>,
    glob_sources: FxHashSet<hir::HirId>,
    externs: Vec<(String, DefId)>,
}

struct Name {
    name: String,
    namespace: Namespace,
    definition: Option<DefId>,
    scope: hir::HirId,
    module: hir::HirId,
}

fn module(cx: &LateContext<'_>, id: hir::HirId) -> hir::HirId {
    cx.tcx
        .local_def_id_to_hir_id(cx.tcx.parent_module(id).to_local_def_id())
}

fn lexical_scope(cx: &LateContext<'_>, id: hir::HirId) -> hir::HirId {
    cx.tcx
        .hir_parent_iter(id)
        .find_map(|(parent, node)| match node {
            hir::Node::Block(_) | hir::Node::Crate(_) | hir::Node::Arm(_) => Some(parent),
            hir::Node::Item(item) if matches!(item.kind, hir::ItemKind::Mod(..)) => Some(parent),
            hir::Node::Expr(expression) if matches!(expression.kind, hir::ExprKind::Closure(..)) => Some(parent),
            _ => None,
        })
        .unwrap_or(hir::CRATE_HIR_ID)
}

fn ancestry(cx: &LateContext<'_>, id: hir::HirId) -> Vec<hir::HirId> {
    std::iter::once(id)
        .chain(cx.tcx.hir_parent_iter(id).map(|(parent, _)| parent))
        .collect()
}
