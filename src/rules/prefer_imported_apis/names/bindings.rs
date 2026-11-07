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
use rustc_span::{
    Span,
    def_id::DefId,
    kw,
};

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
            name: ident.name.to_string(),
            namespace: Namespace::ValueNS,
            definition: None,
            scope,
            module: module(cx, id),
        });
    }

    pub(crate) fn reference(&mut self, cx: &LateContext<'_>, path: &hir::Path<'_>, id: hir::HirId) {
        let mut segments = path
            .segments
            .iter()
            .filter(|segment| segment.ident.name != kw::PathRoot);

        let Some(first) = segments.next() else { return };
        let resolution = if segments.next().is_none() { path.res } else { first.res };
        self.reference_resolution(cx, id, first.ident.name.as_str(), first.ident.span, resolution);
    }

    pub(crate) fn reference_resolution(
        &mut self,
        cx: &LateContext<'_>,
        id: hir::HirId,
        name: &str,
        span: Span,
        resolution: Res,
    ) {
        let Some(namespace) = resolution.ns() else { return };
        self.references.push((
            span,
            Name {
                name: name.to_owned(),
                namespace,
                definition: resolution.opt_def_id(),
                scope: id,
                module: module(cx, id),
            },
        ));
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
