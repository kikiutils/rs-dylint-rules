use pathkit::Path;
use ra_ap_syntax::AstNode;
use rustc_hir::{
    self as hir,
    def::{
        DefKind,
        Res,
    },
};
use rustc_lint::LateContext;
use rustc_span::{
    Span,
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
        ApiUse,
        Kind,
    },
    api,
    resolution::Names,
};
use crate::{
    diagnostic::RuleOutput,
    facts::Facts,
    source::Source,
    workspace::Workspace,
};

#[derive(Default)]
pub(crate) struct Collector {
    facts: Facts<ApiUse>,
    names: Names,
}

impl Collector {
    fn record(
        &mut self,
        cx: &LateContext<'_>,
        workspace: &mut Workspace,
        span: Span,
        id: hir::HirId,
        api: Api,
        kind: Kind,
    ) {
        // Duplicates are removed per file during analysis.
        self.facts.record(cx, workspace, span, id, ApiUse { api, kind });
    }

    fn import(&mut self, cx: &LateContext<'_>, workspace: &mut Workspace, tree: &hir::UseTree<'_>, id: hir::HirId) {
        match tree.kind {
            hir::UseKind::Nested { items } => {
                for (child, id, _) in items {
                    self.import(cx, workspace, child, *id);
                }
            },
            hir::UseKind::Single(binding) => {
                for resolution in tree.prefix.res.iter().flatten() {
                    self.names.binding(cx, id, binding.name.as_str(), *resolution);
                }

                if binding.span.from_expansion() {
                    return;
                }

                for resolution in tree.prefix.res.iter().flatten() {
                    if let Res::Def(_, definition) = resolution
                        && let Some(api) = api(cx, *definition)
                    {
                        self.record(
                            cx,
                            workspace,
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
                for resolution in tree.prefix.res.iter().flatten() {
                    let Res::Def(DefKind::Mod, definition) = resolution else {
                        continue;
                    };

                    self.names.glob(cx, *definition);
                    let children = if let Some(local) = definition.as_local() {
                        cx.tcx.module_children_local(local)
                    } else {
                        cx.tcx.module_children(*definition)
                    };

                    for child in children {
                        if !child
                            .vis
                            .is_accessible_from(cx.tcx.parent_module(id).to_local_def_id().to_def_id(), cx.tcx)
                        {
                            continue;
                        }

                        self.names
                            .binding(cx, id, child.ident.name.as_str(), child.res.map_id(|_| unreachable!()));

                        if let Res::Def(_, definition) = child.res
                            && let Some(api) = api(cx, definition)
                            && !api.allows(child.ident.name.as_str())
                            && !tree.prefix.span.from_expansion()
                        {
                            self.record(cx, workspace, tree.prefix.span, id, api, Kind::Glob);
                        }
                    }
                }
            },
        }
    }

    pub(crate) fn check_item<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        item: &'tcx hir::Item<'tcx>,
    ) {
        if let Some(ident) = item.kind.ident() {
            let definition = item.owner_id.to_def_id();
            self.names.binding(
                cx,
                item.hir_id(),
                ident.name.as_str(),
                Res::Def(cx.tcx.def_kind(definition), definition),
            );
        }

        match item.kind {
            hir::ItemKind::Use(tree) => self.import(cx, workspace, &tree, item.hir_id()),
            hir::ItemKind::ExternCrate(Some(_), name) if !item.span.from_expansion() => {
                if let Some(krate) = cx.tcx.extern_mod_stmt_cnum(item.owner_id.def_id)
                    && cx.tcx.crate_name(krate).as_str() == "anyhow"
                {
                    self.record(cx, workspace, name.span, item.hir_id(), Api::Crate, Kind::CrateAlias);
                }
            },
            _ => {},
        }
    }

    pub(crate) fn check_generic_param<'tcx>(&mut self, cx: &LateContext<'tcx>, param: &'tcx hir::GenericParam<'tcx>) {
        self.names.generic(cx, param);
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

    pub(crate) fn check_path<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        path: &hir::Path<'tcx>,
        id: hir::HirId,
    ) {
        self.names.reference(cx, path, id);
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
                    workspace,
                    path.segments[0].ident.span.to(segment.ident.span),
                    id,
                    api,
                    Kind::Access,
                );

                break;
            }
        }
    }

    pub(crate) fn check_expr<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        expression: &'tcx hir::Expr<'tcx>,
    ) {
        let mut expansion = expression.span.ctxt().outer_expn();
        while expansion != ExpnId::root() {
            let data = expansion.expn_data();
            if matches!(data.kind, ExpnKind::Macro(MacroKind::Bang, _))
                && !data.call_site.from_expansion()
                && let Some(definition) = data.macro_def_id
                && let Some(api @ (Api::Anyhow | Api::Bail | Api::Ensure)) = api(cx, definition)
            {
                self.record(cx, workspace, data.call_site, expression.hir_id, api, Kind::Macro);
            }

            expansion = data.parent;
        }
    }

    pub(crate) fn analyze(
        &self,
        cx: &LateContext<'_>,
        path: &Path,
        source: &Source,
        workspace: &Workspace,
    ) -> RuleOutput {
        let facts: Vec<_> = self.facts.file(path, &workspace.modules).collect();
        analysis::analyze(
            source.parsed.syntax(),
            &source.text,
            facts.iter().map(|fact| (fact.range, &fact.value)),
            |request| {
                facts
                    .iter()
                    .find_map(|fact| {
                        (fact.range.contains_range(request.at) || request.at.contains_range(fact.range))
                            .then_some(fact.id)
                    })
                    .is_some_and(|id| self.names.safe(cx, workspace, id, &request))
            },
        )
    }
}
