use std::cell::OnceCell;

use kikiutils::types::fx_collections::FxHashSet;
use pathkit::Path;
use ra_ap_syntax::AstNode;
use rustc_hir::{
    self as hir,
    def::Res,
};
use rustc_lint::LateContext;
use rustc_span::{
    Span,
    def_id::{
        CRATE_DEF_INDEX,
        DefId,
    },
    hygiene::ExpnId,
};

mod accesses;
mod derives;
mod imports;
mod index;

use super::{
    analysis,
    model::Event,
    names::Names,
    policy::{
        Api,
        Catalog,
    },
};
use crate::{
    diagnostic::RuleOutput,
    facts::Facts,
    source::Source,
    workspace::Workspace,
};

#[derive(Default)]
pub(crate) struct Collector {
    catalog: OnceCell<Catalog>,
    facts: Facts<Event>,
    derives: Facts<&'static [&'static str]>,
    names: Names,
    macro_calls: FxHashSet<ExpnId>,
}

impl Collector {
    fn catalog(&self, cx: &LateContext<'_>) -> &Catalog {
        self.catalog.get_or_init(|| Catalog::build(cx))
    }

    fn api(&self, cx: &LateContext<'_>, definition: DefId) -> Option<Api> {
        self.catalog(cx)
            .api(definition)
            .or_else(|| {
                matches!(cx.tcx.def_kind(definition), hir::def::DefKind::Ctor(..))
                    .then(|| cx.tcx.opt_parent(definition))
                    .flatten()
                    .and_then(|parent| self.catalog(cx).api(parent))
            })
            .cloned()
    }

    fn record(&mut self, cx: &LateContext<'_>, workspace: &mut Workspace, span: Span, id: hir::HirId, event: Event) {
        self.facts.record(cx, workspace, span, id, event);
    }

    pub(crate) fn check_item<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        item: &'tcx hir::Item<'tcx>,
    ) {
        self.expansion_paths(cx, workspace, item.span, item.hir_id());
        self.derive(cx, workspace, item);
        if !matches!(item.kind, hir::ItemKind::ExternCrate(..))
            && let Some(ident) = item.kind.ident()
        {
            let definition = item.owner_id.to_def_id();
            self.names.binding(
                cx,
                item.hir_id(),
                ident.name.as_str(),
                Res::Def(cx.tcx.def_kind(definition), definition),
            );
        }

        if let hir::ItemKind::Struct(ident, _, data) = item.kind
            && let Some(constructor) = data.ctor_def_id()
        {
            let definition = constructor.to_def_id();
            self.names.binding(
                cx,
                item.hir_id(),
                ident.name.as_str(),
                Res::Def(cx.tcx.def_kind(definition), definition),
            );
        }

        match item.kind {
            hir::ItemKind::Use(tree) => self.import(cx, workspace, &tree, item.hir_id()),
            hir::ItemKind::ExternCrate(original, ident) => {
                if let Some(krate) = cx.tcx.extern_mod_stmt_cnum(item.owner_id.def_id) {
                    let definition = DefId {
                        krate,
                        index: CRATE_DEF_INDEX,
                    };

                    self.names.external(cx, item.hir_id(), ident.name.as_str(), definition);
                    self.names.binding(
                        cx,
                        item.hir_id(),
                        ident.name.as_str(),
                        Res::Def(hir::def::DefKind::Mod, definition),
                    );

                    if !ident.span.from_expansion()
                        && let Some(qualifier) = self.catalog(cx).qualifier(definition).cloned()
                    {
                        self.record(
                            cx,
                            workspace,
                            ident.span,
                            item.hir_id(),
                            Event::NamespaceImport {
                                qualifier,
                                name: ident.name.to_string(),
                                renamed: original.is_some(),
                            },
                        );
                    }
                }
            },
            _ => {},
        }
    }

    pub(crate) fn check_foreign_item<'tcx>(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ForeignItem<'tcx>) {
        let definition = item.owner_id.to_def_id();
        self.names.binding(
            cx,
            item.hir_id(),
            item.ident.name.as_str(),
            Res::Def(cx.tcx.def_kind(definition), definition),
        );
    }

    pub(crate) fn check_generic_param<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        parameter: &'tcx hir::GenericParam<'tcx>,
    ) {
        self.names.generic(cx, parameter);
    }

    pub(crate) fn check_pat<'tcx>(&mut self, cx: &LateContext<'tcx>, pattern: &'tcx hir::Pat<'tcx>) {
        self.names.pattern(cx, pattern);
    }

    pub(crate) fn analyze(
        &self,
        cx: &LateContext<'_>,
        path: &Path,
        source: &Source,
        workspace: &Workspace,
    ) -> RuleOutput {
        let facts: Vec<_> = self.facts.file(path, &workspace.modules).collect();
        let index = index::FactIndex::new(&facts);
        let references: FxHashSet<_> = self.names.reference_ranges(cx, workspace, path).collect();
        let derives = self.derives.resolved(path, &workspace.modules);
        analysis::analyze(
            source.parsed.syntax(),
            &source.text,
            source.edition,
            facts.iter().map(|fact| (fact.range, fact.id, &fact.value)),
            &super::fix::Checks {
                name: &|request| {
                    let source_imports = request.replacement.as_ref().map_or(&[][..], |replacement| {
                        index.source_imports(replacement.definition, request.name)
                    });

                    let origins = index.origins(request.fact);
                    !origins.is_empty()
                        && origins
                            .iter()
                            .all(|&id| self.names.safe(cx, workspace, path, id, &request, source_imports))
                },
                remove: &|request| {
                    let origins = index.import_origins(request.origin);
                    !origins.is_empty()
                        && origins.iter().all(|&origin| {
                            self.names.removable(
                                cx,
                                workspace,
                                path,
                                &super::names::RemovalRequest { origin, ..request },
                            )
                        })
                },
                protected: &|node| {
                    analysis::attributes::protected(node, &|range| derives.get(&range).copied().flatten())
                },
                reference: &|range| references.contains(&range),
                depends: &|request| {
                    index.import_origins(request.origin).iter().any(|&origin| {
                        self.names
                            .depends(cx, workspace, path, &super::names::RemovalRequest { origin, ..request })
                    })
                },
            },
        )
    }
}
