use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::hygiene::{
    ExpnKind,
    MacroKind,
};

use super::Collector;
use crate::workspace::Workspace;

impl Collector {
    pub(super) fn derive(&mut self, cx: &LateContext<'_>, workspace: &mut Workspace, item: &hir::Item<'_>) {
        if !matches!(item.kind, hir::ItemKind::Impl(_)) {
            return;
        }

        let expansion = item.span.ctxt().outer_expn_data();
        if !matches!(expansion.kind, ExpnKind::Macro(MacroKind::Derive, _)) || expansion.call_site.from_expansion() {
            return;
        }

        let Some(definition) = expansion.macro_def_id else {
            return;
        };

        let Some(helpers) = crate::derives::helpers(
            cx.tcx.item_name(definition).as_str(),
            cx.tcx.crate_name(definition.krate).as_str(),
            cx.tcx.is_builtin_derived(item.owner_id.to_def_id()),
        ) else {
            return;
        };

        let Some(adt) = cx
            .tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .skip_norm_wip()
            .peel_refs()
            .ty_adt_def()
        else {
            return;
        };

        let Some(local) = adt.did().as_local() else { return };
        if cx.tcx.def_span(local).from_expansion() {
            return;
        }

        self.derives.record(
            cx,
            workspace,
            expansion.call_site,
            cx.tcx.local_def_id_to_hir_id(local),
            helpers,
        );
    }
}
