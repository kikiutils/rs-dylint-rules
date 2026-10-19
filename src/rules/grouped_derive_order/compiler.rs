use std::collections::BTreeMap;

use pathkit::Path;
use ra_ap_syntax::{
    AstNode,
    TextRange,
};
use rustc_hir as hir;
use rustc_lint::{
    LateContext,
    LateLintPass,
    LintContext,
};
use rustc_lint_defs::{
    declare_lint,
    impl_lint_pass,
};
use rustc_span::{
    Span,
    hygiene::{
        ExpnKind,
        MacroKind,
    },
};

use super::analysis;

declare_lint! { pub KIKI_GROUPED_DERIVE_ORDER, Warn, "group supported derives in a consistent order" }

pub(super) fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[KIKI_GROUPED_DERIVE_ORDER]);
    store.register_late_lint_pass(Box::new(|_| Box::<GroupedDeriveOrder>::default()));
}

#[derive(Default)]
struct GroupedDeriveOrder {
    files: BTreeMap<(Path, u32), FileFacts>,
    sources: crate::source::ModuleSources,
}

struct FileFacts {
    anchor: Span,
    derives: BTreeMap<(u32, u32), (Option<analysis::SortKey>, hir::HirId)>,
}

impl_lint_pass!(GroupedDeriveOrder => [KIKI_GROUPED_DERIVE_ORDER]);

impl<'tcx> LateLintPass<'tcx> for GroupedDeriveOrder {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx hir::Mod<'tcx>, id: rustc_span::def_id::LocalModId) {
        self.sources.record(
            cx,
            module.spans.inner_span,
            id.to_local_def_id().local_def_index.as_u32(),
        );
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
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
        let builtin = cx.tcx.is_builtin_derived(item.owner_id.to_def_id());
        let supported = match cx.tcx.crate_name(definition.krate).as_str() {
            "serde_derive" => {
                matches!(name.as_str(), "Serialize" | "Deserialize")
                    && cx.tcx.is_automatically_derived(item.owner_id.to_def_id())
            },
            "is_empty_derive" => {
                name.as_str() == "IsEmpty" && cx.tcx.is_automatically_derived(item.owner_id.to_def_id())
            },
            "strum_macros" => {
                matches!(name.as_str(), "EnumIs" | "EnumCount" | "VariantNames" | "VariantArray")
                    && cx.tcx.is_automatically_derived(item.owner_id.to_def_id())
            },
            "serde_repr" => matches!(name.as_str(), "Serialize_repr" | "Deserialize_repr"),
            _ => false,
        };
        if !builtin && !supported {
            return;
        }
        let Some(key) = analysis::sort_key(name.as_str()) else {
            return;
        };
        let Some((path, range)) = crate::source::locate(cx, expansion.call_site) else {
            return;
        };
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
        // Ignore generated helper types; lint levels belong to the user's type.
        if cx.tcx.def_span(local).from_expansion() {
            return;
        }
        let id = cx.tcx.local_def_id_to_hir_id(local);
        self.files
            .entry((
                path,
                cx.tcx.parent_module(id).to_local_def_id().local_def_index.as_u32(),
            ))
            .or_insert_with(|| FileFacts {
                anchor: expansion.call_site,
                derives: BTreeMap::new(),
            })
            .derives
            .entry((range.start().into(), range.end().into()))
            .and_modify(|(existing, _)| {
                if *existing != Some(key) {
                    *existing = None;
                }
            })
            .or_insert((Some(key), id));
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for ((path, module), facts) in &self.files {
            if !self.sources.contains(path, *module) {
                continue;
            }
            let source = match crate::source::Source::load(cx, path, facts.anchor) {
                Ok(source) => source,
                Err(error) => {
                    cx.sess().dcx().span_err(facts.anchor, format!("{error:#}"));
                    continue;
                },
            };
            let output = analysis::analyze(source.parsed.syntax(), |range| {
                facts
                    .derives
                    .get(&(range.start().into(), range.end().into()))
                    .and_then(|(key, _)| *key)
            });
            for issue in output.issues {
                let diagnostic = &issue.diagnostics[0];
                let Some(id) = facts.derives.iter().find_map(|(&(start, end), (_, id))| {
                    diagnostic
                        .range
                        .contains_range(TextRange::new(start.into(), end.into()))
                        .then_some(*id)
                }) else {
                    continue;
                };
                issue.emit(
                    cx,
                    KIKI_GROUPED_DERIVE_ORDER,
                    id,
                    &source,
                    "reorder this derive list",
                    self.sources.can_fix(path),
                );
            }
        }
    }
}
