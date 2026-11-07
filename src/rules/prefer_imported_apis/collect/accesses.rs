use rustc_hir::{
    self as hir,
    def::Res,
};
use rustc_lint::LateContext;
use rustc_span::hygiene::{
    ExpnId,
    ExpnKind,
    MacroKind,
};

use super::{
    super::model::{
        Event,
        UseKind,
    },
    Collector,
};
use crate::workspace::Workspace;

impl Collector {
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
                && let Some(api) = self.api(cx, definition)
            {
                self.record(
                    cx,
                    workspace,
                    path.span.with_hi(segment.ident.span.hi()),
                    id,
                    Event::Use {
                        kind: UseKind::Path,
                        api,
                    },
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
        // HIR visits the outer expansion expression first. Its descendants are
        // not independent source invocations, unlike distinct expansion IDs.
        let mut expansion = expression.span.ctxt().outer_expn();
        while expansion != ExpnId::root() {
            let data = expansion.expn_data();
            if matches!(data.kind, ExpnKind::Macro(MacroKind::Bang, _))
                && !data.call_site.from_expansion()
                && let Some(definition) = data.macro_def_id
                && let Some(api) = self.api(cx, definition)
                && self.macro_calls.insert(expansion)
            {
                self.record(
                    cx,
                    workspace,
                    data.call_site,
                    expression.hir_id,
                    Event::Use {
                        kind: UseKind::Macro,
                        api,
                    },
                );
            }

            expansion = data.parent;
        }

        if let hir::ExprKind::MethodCall(segment, _, _, _) = expression.kind
            && !segment.ident.span.from_expansion()
            && let Some(definition) = cx.typeck_results().type_dependent_def_id(expression.hir_id)
            && let Some(trait_definition) = cx.tcx.trait_of_assoc(definition).or_else(|| {
                cx.tcx
                    .trait_item_of(definition)
                    .and_then(|item| cx.tcx.trait_of_assoc(item))
            })
            && let Some(api) = self.api(cx, trait_definition)
        {
            self.record(
                cx,
                workspace,
                segment.ident.span,
                expression.hir_id,
                Event::Use {
                    kind: UseKind::Trait,
                    api,
                },
            );
        }
    }
}
