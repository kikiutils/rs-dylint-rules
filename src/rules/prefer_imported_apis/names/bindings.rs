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
use rustc_span::def_id::DefId;

use super::{
    Name,
    Names,
    lexical_scope,
    module,
};

impl Names {
    pub(crate) fn binding(&mut self, cx: &LateContext<'_>, id: hir::HirId, name: &str, resolution: Res) {
        let Some(namespace) = resolution.ns() else { return };
        if let Res::Def(DefKind::Macro(kinds), _) = resolution
            && !kinds.contains(MacroKinds::BANG)
        {
            return;
        }

        self.bindings.push(Name {
            id,
            name: name.to_owned(),
            namespace,
            definition: resolution.opt_def_id(),
            scope: lexical_scope(cx, id),
            module: module(cx, id),
        });
    }

    pub(crate) fn generic(&mut self, cx: &LateContext<'_>, parameter: &hir::GenericParam<'_>) {
        let namespace = match parameter.kind {
            hir::GenericParamKind::Type { .. } => Namespace::TypeNS,
            hir::GenericParamKind::Const { .. } => Namespace::ValueNS,
            hir::GenericParamKind::Lifetime { .. } => return,
        };

        self.bindings.push(Name {
            id: parameter.hir_id,
            name: parameter.name.ident().name.to_string(),
            namespace,
            definition: None,
            scope: cx.tcx.hir_get_parent_item(parameter.hir_id).into(),
            module: module(cx, parameter.hir_id),
        });
    }

    pub(crate) fn pattern(&mut self, cx: &LateContext<'_>, pattern: &hir::Pat<'_>) {
        let hir::PatKind::Binding(_, id, ident, _) = pattern.kind else {
            return;
        };

        // Parameters belong to their function/closure, not the module containing
        // the declaration. Ordinary let bindings belong to their lexical block.
        let mut parameter = false;
        let scope = cx
            .tcx
            .hir_parent_iter(pattern.hir_id)
            .find_map(|(parent, node)| {
                if matches!(node, hir::Node::Param(_)) {
                    parameter = true;
                }

                match node {
                    hir::Node::Expr(expression) if matches!(expression.kind, hir::ExprKind::Closure(..)) => {
                        Some(parent)
                    },
                    hir::Node::Item(_) | hir::Node::ImplItem(_) | hir::Node::TraitItem(_) if parameter => Some(parent),
                    hir::Node::Block(_) | hir::Node::Arm(_) if !parameter => Some(parent),
                    _ => None,
                }
            })
            .unwrap_or_else(|| lexical_scope(cx, pattern.hir_id));

        self.bindings.push(Name {
            id,
            name: ident.name.to_string(),
            namespace: Namespace::ValueNS,
            definition: None,
            scope,
            module: module(cx, id),
        });
    }

    pub(crate) fn glob(&mut self, cx: &LateContext<'_>, definition: DefId) {
        if let Some(local) = definition.as_local() {
            self.glob_sources.insert(cx.tcx.local_def_id_to_hir_id(local));
        }
    }

    pub(crate) fn external(&mut self, cx: &LateContext<'_>, id: hir::HirId, name: &str, definition: DefId) {
        if module(cx, id) == hir::CRATE_HIR_ID {
            self.externs.push((name.to_owned(), definition));
        }
    }
}
