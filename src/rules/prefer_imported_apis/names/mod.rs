use std::cell::OnceCell;

use kikiutils::types::fx_collections::FxHashSet;
use ra_ap_syntax::TextRange;
use rustc_hir::{
    self as hir,
    def::{
        Namespace,
        Res,
    },
};
use rustc_lint::LateContext;
use rustc_span::{
    Ident,
    Span,
    def_id::DefId,
};

mod bindings;
mod paths;
mod safety;

pub(super) use self::safety::external_crate;

pub(super) struct ReplacedName<'a> {
    pub(super) definition: DefId,
    pub(super) ranges: &'a [TextRange],
}

#[derive(Clone, Copy)]
pub(super) struct RemovalRequest<'a> {
    pub(super) name: &'a str,
    pub(super) definition: DefId,
    pub(super) namespace: Namespace,
    pub(super) range: TextRange,
    pub(super) scope: TextRange,
    pub(super) origin: hir::HirId,
    pub(super) replacements: &'a [TextRange],
    // Source paths reused by new imports still depend on their original bindings.
    pub(super) retained: &'a [TextRange],
}

impl RemovalRequest<'_> {
    pub(super) fn covered(&self, range: TextRange) -> bool {
        self.replacements
            .iter()
            .any(|replacement| replacement.contains_range(range))
            && !self.retained.iter().any(|retained| retained.contains_range(range))
    }
}

pub(super) struct NameRequest<'a> {
    pub(super) fact: super::model::FactKey,
    pub(super) at: TextRange,
    pub(super) name: &'a str,
    pub(super) definition: Option<DefId>,
    pub(super) namespace: Namespace,
    pub(super) target: TextRange,
    pub(super) module_target: bool,
    pub(super) reuse: bool,
    pub(super) external: bool,
    pub(super) replacement: Option<ReplacedName<'a>>,
}

#[derive(Default)]
pub(super) struct Names {
    bindings: Vec<Name>,
    paths: Vec<NamePath>,
    references: OnceCell<Vec<Reference>>,
    glob_sources: FxHashSet<hir::HirId>,
    externs: Vec<(String, DefId)>,
}

#[derive(Clone)]
pub(super) struct Segment {
    ident: Ident,
    resolutions: Vec<Res>,
}

struct NamePath {
    id: hir::HirId,
    segments: Vec<Segment>,
}

struct Reference {
    span: Span,
    name: Name,
    lexical: bool,
    via: Option<DefId>,
}

struct Name {
    id: hir::HirId,
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
