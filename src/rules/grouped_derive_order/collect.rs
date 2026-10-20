use pathkit::Path;
use ra_ap_syntax::{
    AstNode,
    TextRange,
};
use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::hygiene::{
    ExpnKind,
    MacroKind,
};

use super::analysis;
use crate::{
    diagnostic::RuleOutput,
    facts::Facts,
    source::Source,
    workspace::Workspace,
};

#[derive(Default)]
pub(crate) struct Collector {
    facts: Facts<analysis::SortKey>,
}

impl Collector {
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, workspace: &mut Workspace, item: &hir::Item<'_>) {
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

        let name = cx.tcx.item_name(definition);
        let Some(derive) = super::DERIVES.iter().find(|derive| derive.name == name.as_str()) else {
            return;
        };

        let supported = match derive.krate {
            None => cx.tcx.is_builtin_derived(item.owner_id.to_def_id()),
            Some(krate) => {
                cx.tcx.crate_name(definition.krate).as_str() == krate
                    && (!derive.check_automatically_derived
                        || cx.tcx.is_automatically_derived(item.owner_id.to_def_id()))
            },
        };

        if !supported {
            return;
        }

        let Some(adt) = cx
            .tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .skip_norm_wip()
            .ty_adt_def()
        else {
            return;
        };

        let Some(local) = adt.did().as_local() else { return };
        // Ignore generated helper types; only collect derives on the original type.
        if cx.tcx.def_span(local).from_expansion() {
            return;
        }

        let id = cx.tcx.local_def_id_to_hir_id(local);
        self.facts.record(cx, workspace, expansion.call_site, id, derive.key);
    }

    pub(crate) fn analyze(&self, path: &Path, source: &Source, workspace: &Workspace) -> RuleOutput {
        let derives = self.facts.resolved(path, &workspace.modules);
        analysis::analyze(source.parsed.syntax(), &source.text, |range: TextRange| {
            derives.get(&range).copied().flatten()
        })
    }
}
