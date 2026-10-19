use std::collections::{
    BTreeMap,
    BTreeSet,
};

use pathkit::Path;
use ra_ap_syntax::AstNode;
use rustc_hir::{
    self as hir,
    def::{
        DefKind,
        Res,
    },
};
use rustc_lint::{
    LateContext,
    LateLintPass,
};
use rustc_lint_defs::{
    declare_lint,
    impl_lint_pass,
};
use rustc_span::{
    Span,
    def_id::{
        CRATE_DEF_INDEX,
        DefId,
        LocalModId,
    },
    hygiene::{
        ExpnId,
        ExpnKind,
        MacroKind,
    },
};

use super::{
    Api,
    analysis::{
        self,
        Fact,
        Kind,
    },
};

declare_lint! { pub KIKI_PREFER_IMPORTED_APIS, Warn, "import selected APIs using their prescribed names" }

pub(super) fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[KIKI_PREFER_IMPORTED_APIS]);
    store.register_late_lint_pass(Box::new(|_| Box::<PreferImportedApis>::default()));
}

#[derive(Default)]
struct PreferImportedApis {
    files: BTreeMap<(Path, u32), FileFacts>,
    sources: crate::source::ModuleSources,
    local_macros: BTreeSet<String>,
}

struct FileFacts {
    anchor: Span,
    facts: Vec<(Fact, hir::HirId)>,
}

fn api(cx: &LateContext<'_>, definition: DefId) -> Option<Api> {
    if cx.tcx.crate_name(definition.krate).as_str() != "anyhow" {
        return None;
    }
    if definition.index == CRATE_DEF_INDEX {
        return Some(Api::Crate);
    }
    if matches!(cx.tcx.def_kind(definition), DefKind::Macro(_)) {
        return match cx.tcx.item_name(definition).as_str() {
            "anyhow" => Some(Api::Anyhow),
            "bail" => Some(Api::Bail),
            "ensure" => Some(Api::Ensure),
            _ => None,
        };
    }
    if cx
        .tcx
        .opt_parent(definition)
        .is_none_or(|parent| parent.index != CRATE_DEF_INDEX)
    {
        return None;
    }
    match cx.tcx.item_name(definition).as_str() {
        "Result" => Some(Api::Result),
        "Error" => Some(Api::Error),
        _ => None,
    }
}

impl PreferImportedApis {
    fn record(&mut self, cx: &LateContext<'_>, span: Span, id: hir::HirId, api: Api, kind: Kind) {
        let Some((path, range)) = crate::source::locate(cx, span) else {
            return;
        };
        let facts = &mut self
            .files
            .entry((
                path,
                cx.tcx.parent_module(id).to_local_def_id().local_def_index.as_u32(),
            ))
            .or_insert_with(|| FileFacts {
                anchor: span,
                facts: Vec::new(),
            })
            .facts;
        let fact = Fact { range, api, kind };
        if !facts.iter().any(|(existing, _)| *existing == fact) {
            facts.push((fact, id));
        }
    }

    fn import(&mut self, cx: &LateContext<'_>, tree: &hir::UseTree<'_>, id: hir::HirId) {
        match tree.kind {
            hir::UseKind::Nested { items } => {
                for (child, id, _) in items {
                    self.import(cx, child, *id);
                }
            },
            hir::UseKind::Single(binding) => {
                if binding.span.from_expansion() {
                    return;
                }
                for resolution in tree.prefix.res.iter().flatten() {
                    if let Res::Def(_, definition) = resolution
                        && let Some(api) = api(cx, *definition)
                    {
                        self.record(
                            cx,
                            binding.span,
                            id,
                            api,
                            Kind::Import {
                                name: binding.name.to_string(),
                            },
                        );
                    }
                }
            },
            hir::UseKind::Glob => {
                if tree.prefix.span.from_expansion() {
                    return;
                }
                for resolution in tree.prefix.res.iter().flatten() {
                    let Res::Def(DefKind::Mod, definition) = resolution else {
                        continue;
                    };
                    let children = if let Some(local) = definition.as_local() {
                        cx.tcx.module_children_local(local)
                    } else {
                        cx.tcx.module_children(*definition)
                    };
                    for child in children {
                        if child
                            .vis
                            .is_accessible_from(cx.tcx.parent_module(id).to_local_def_id().to_def_id(), cx.tcx)
                            && let Res::Def(_, definition) = child.res
                            && let Some(api) = api(cx, definition)
                            && !api.allows(child.ident.name.as_str())
                        {
                            self.record(cx, tree.prefix.span, id, api, Kind::Glob);
                        }
                    }
                }
            },
        }
    }
}

impl_lint_pass!(PreferImportedApis => [KIKI_PREFER_IMPORTED_APIS]);

impl<'tcx> LateLintPass<'tcx> for PreferImportedApis {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx hir::Mod<'tcx>, id: LocalModId) {
        self.sources.record(
            cx,
            module.spans.inner_span,
            id.to_local_def_id().local_def_index.as_u32(),
        );
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        match item.kind {
            hir::ItemKind::Use(tree) => self.import(cx, &tree, item.hir_id()),
            hir::ItemKind::Macro(name, ..) => {
                self.local_macros.insert(name.name.to_string());
            },
            hir::ItemKind::ExternCrate(Some(_), name) if !item.span.from_expansion() => {
                if let Some(krate) = cx.tcx.extern_mod_stmt_cnum(item.owner_id.def_id)
                    && cx.tcx.crate_name(krate).as_str() == "anyhow"
                {
                    self.record(cx, name.span, item.hir_id(), Api::Crate, Kind::CrateAlias);
                }
            },
            _ => {},
        }
    }

    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &hir::Path<'tcx>, id: hir::HirId) {
        if path.span.from_expansion() {
            return;
        }
        for (index, segment) in path.segments.iter().enumerate() {
            let resolution = if index + 1 == path.segments.len() {
                path.res
            } else {
                segment.res
            };
            if let Res::Def(_, definition) = resolution
                && let Some(api) = api(cx, definition)
                && api != Api::Crate
            {
                self.record(
                    cx,
                    path.segments[0].ident.span.to(segment.ident.span),
                    id,
                    api,
                    Kind::Access,
                );
                break;
            }
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx hir::Expr<'tcx>) {
        let mut expansion = expression.span.ctxt().outer_expn();
        while expansion != ExpnId::root() {
            let data = expansion.expn_data();
            if matches!(data.kind, ExpnKind::Macro(MacroKind::Bang, _))
                && !data.call_site.from_expansion()
                && let Some(definition) = data.macro_def_id
                && let Some(api @ (Api::Anyhow | Api::Bail | Api::Ensure)) = api(cx, definition)
            {
                self.record(cx, data.call_site, expression.hir_id, api, Kind::Macro);
            }
            expansion = data.parent;
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (key @ (path, _), facts) in &self.files {
            // include! may provide a Rust expression rather than an item source
            // file. Only analyze the module's own physical source in this rule.
            if !self.sources.contains(path, key.1) {
                continue;
            }
            let Some(source) = crate::source::Source::load(cx, path, facts.anchor) else {
                continue;
            };
            let output = analysis::analyze(
                source.parsed.syntax(),
                &facts.facts.iter().map(|(fact, _)| fact.clone()).collect::<Vec<_>>(),
                &self.local_macros,
            );
            for issue in output.issues {
                let diagnostic = &issue.diagnostics[0];
                let Some(id) = facts.facts.iter().find_map(|(fact, id)| {
                    (fact.range.contains_range(diagnostic.range) || diagnostic.range.contains_range(fact.range))
                        .then_some(*id)
                }) else {
                    continue;
                };
                issue.emit(
                    cx,
                    KIKI_PREFER_IMPORTED_APIS,
                    id,
                    &source,
                    "import the API and use its prescribed name",
                    self.sources.can_fix(path),
                );
            }
        }
    }
}
