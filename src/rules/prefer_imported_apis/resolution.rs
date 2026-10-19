use std::collections::HashSet;

use ra_ap_syntax::TextRange;
use rustc_hir::{
    self as hir,
    def::{
        DefKind,
        MacroKinds,
        Namespace,
        Res,
    },
};
use rustc_lint::LateContext;

use super::{
    Api,
    compiler::api,
};

pub(super) struct NameRequest<'a> {
    pub(super) at: TextRange,
    pub(super) api: Api,
    pub(super) name: &'a str,
    pub(super) target: TextRange,
    pub(super) module_target: bool,
    pub(super) reuse: bool,
}

#[derive(Default)]
pub(super) struct Names {
    bindings: Vec<Name>,
    references: Vec<Name>,
    glob_sources: HashSet<hir::HirId>,
}

struct Name {
    name: String,
    namespace: Namespace,
    api: Option<Api>,
    scope: hir::HirId,
    module: hir::HirId,
}

fn candidate(name: &str, namespace: Namespace) -> bool {
    match namespace {
        Namespace::TypeNS => matches!(name, "Result" | "AnyhowResult" | "AnyhowError"),
        Namespace::MacroNS => matches!(name, "anyhow" | "bail" | "ensure"),
        Namespace::ValueNS => false,
    }
}

fn module(cx: &LateContext<'_>, id: hir::HirId) -> hir::HirId {
    cx.tcx
        .local_def_id_to_hir_id(cx.tcx.parent_module(id).to_local_def_id())
}

fn lexical_scope(cx: &LateContext<'_>, id: hir::HirId) -> hir::HirId {
    cx.tcx
        .hir_parent_iter(id)
        .find_map(|(parent, node)| match node {
            hir::Node::Block(_) | hir::Node::Crate(_) => Some(parent),
            hir::Node::Item(item) if matches!(item.kind, hir::ItemKind::Mod(..)) => Some(parent),
            _ => None,
        })
        .unwrap_or(hir::CRATE_HIR_ID)
}

fn ancestry(cx: &LateContext<'_>, id: hir::HirId) -> Vec<hir::HirId> {
    std::iter::once(id)
        .chain(cx.tcx.hir_parent_iter(id).map(|(id, _)| id))
        .collect()
}

impl Names {
    pub(super) fn glob(&mut self, cx: &LateContext<'_>, definition: rustc_span::def_id::DefId) {
        if let Some(local) = definition.as_local() {
            self.glob_sources.insert(cx.tcx.local_def_id_to_hir_id(local));
        }
    }

    pub(super) fn binding(&mut self, cx: &LateContext<'_>, id: hir::HirId, name: &str, res: Res) {
        let Some(namespace) = res.ns() else { return };
        if !candidate(name, namespace) {
            return;
        }
        if let Res::Def(DefKind::Macro(kinds), _) = res
            && !kinds.contains(MacroKinds::BANG)
        {
            return;
        }
        self.bindings.push(Name {
            name: name.to_owned(),
            namespace,
            api: if let Res::Def(_, definition) = res {
                api(cx, definition)
            } else {
                None
            },
            scope: lexical_scope(cx, id),
            module: module(cx, id),
        });
    }

    pub(super) fn generic(&mut self, cx: &LateContext<'_>, param: &hir::GenericParam<'_>) {
        if !matches!(param.kind, hir::GenericParamKind::Type { .. }) {
            return;
        }
        let name = param.name.ident().name.to_string();
        if !candidate(&name, Namespace::TypeNS) {
            return;
        }
        self.bindings.push(Name {
            name,
            namespace: Namespace::TypeNS,
            api: None,
            scope: cx.tcx.hir_get_parent_item(param.hir_id).into(),
            module: module(cx, param.hir_id),
        });
    }

    pub(super) fn reference(&mut self, cx: &LateContext<'_>, path: &hir::Path<'_>, id: hir::HirId) {
        let Some(first) = path.segments.first() else { return };
        let resolution = if path.segments.len() == 1 { path.res } else { first.res };
        let Some(namespace) = resolution.ns() else { return };
        let name = path.segments[0].ident.name.to_string();
        if !candidate(&name, namespace) {
            return;
        }
        self.references.push(Name {
            name,
            namespace,
            api: if let Res::Def(_, definition) = resolution {
                api(cx, definition)
            } else {
                None
            },
            scope: id,
            module: module(cx, id),
        });
    }

    pub(super) fn safe(&self, cx: &LateContext<'_>, id: hir::HirId, request: &NameRequest<'_>) -> bool {
        let namespace = match request.api {
            Api::Anyhow | Api::Bail | Api::Ensure => Namespace::MacroNS,
            _ => Namespace::TypeNS,
        };
        let own_module = module(cx, id);
        let parents = ancestry(cx, id);
        let target = if request.module_target {
            own_module
        } else {
            let Some(target) = cx.tcx.hir_parent_iter(id).find_map(|(id, node)| {
                let hir::Node::Block(block) = node else { return None };
                crate::source::locate(cx, block.span).and_then(|(_, range)| (range == request.target).then_some(id))
            }) else {
                return false;
            };
            target
        };
        // A module import can flow through another scope's glob, including
        // child-file super::* consumers. Do not mutate those unseen bindings.
        if !request.reuse && request.module_target && self.glob_sources.contains(&target) {
            return false;
        }
        for binding in &self.bindings {
            if binding.name != request.name || binding.namespace != namespace {
                continue;
            }
            // Ordinary item and import bindings do not flow into child modules.
            if binding.module != own_module {
                continue;
            }
            if (parents.contains(&binding.scope) || binding.scope == target)
                && !(request.reuse && binding.api == Some(request.api))
            {
                return false;
            }
        }
        // A new import must not capture existing paths, including prelude names.
        request.reuse
            || !self.references.iter().any(|reference| {
                reference.name == request.name
                    && reference.namespace == namespace
                    && reference.module == own_module
                    && reference.api != Some(request.api)
                    && ancestry(cx, reference.scope).contains(&target)
            })
    }
}
