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
use rustc_span::kw;

use super::analysis::{
    self,
    Import,
    Segment,
};
use crate::{
    diagnostic::RuleOutput,
    facts::Facts,
    source::Source,
    workspace::Workspace,
};

#[derive(Default)]
pub(crate) struct Collector {
    facts: Facts<Import>,
}

impl Collector {
    pub(crate) fn check_item<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        item: &hir::Item<'tcx>,
    ) {
        if let hir::ItemKind::Use(tree) = item.kind {
            self.import(cx, workspace, &tree, item.hir_id(), &[]);
        }
    }

    fn import<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        workspace: &mut Workspace,
        tree: &hir::UseTree<'tcx>,
        id: hir::HirId,
        parent: &[&hir::PathSegment<'tcx>],
    ) {
        let mut segments = parent.to_vec();
        segments.extend(tree.prefix.segments);
        if let hir::UseKind::Nested { items } = tree.kind {
            for (child, id, _) in items {
                self.import(cx, workspace, child, *id, &segments);
            }

            return;
        }

        let Some(&first) = segments.first() else { return };
        if segments.iter().any(|segment| segment.ident.span.from_expansion()) {
            return;
        }

        let module = cx.tcx.parent_module(id).to_local_def_id();
        let mut ancestors = vec![module];
        let mut current = module;
        loop {
            let parent = cx.tcx.parent_module_from_def_id(current).to_local_def_id();
            if parent == current {
                break;
            }

            ancestors.push(parent);
            current = parent;
        }

        ancestors.reverse();
        let depth = ancestors.len() - 1;
        let (mut base, mut consumed) = match first.ident.name {
            kw::Crate => (0, 1),
            kw::SelfLower => (depth, 1),
            kw::Super => {
                let count = segments
                    .iter()
                    .take_while(|segment| segment.ident.name == kw::Super)
                    .count();

                let Some(base) = depth.checked_sub(count) else { return };
                (base, count)
            },
            // Absolute paths in supported editions start in the extern prelude.
            kw::PathRoot => return,
            _ => {
                let local = |res: Res| matches!(res, Res::Def(_, definition) if definition.is_local());
                let resolutions: Vec<_> = if segments.len() == 1 {
                    tree.prefix.res.iter().flatten().copied().collect()
                } else {
                    vec![first.res]
                };

                if !resolutions.iter().copied().any(local) {
                    return;
                }

                // A block-local binding is not necessarily visible through self::.
                if !cx.tcx.module_children_local(module).iter().any(|child| {
                    child.ident.name == first.ident.name && resolutions.contains(&child.res.map_id(|_| unreachable!()))
                }) {
                    return;
                }

                (depth, 0)
            },
        };

        // Consume only declarations on the real ancestry, with their original
        // names. A re-export/alias must remain part of the written route.
        while base < depth && consumed < segments.len() {
            let next = ancestors[base + 1];
            let segment = segments[consumed];
            if segment.ident.name != cx.tcx.item_name(next.to_def_id())
                || segment.res != Res::Def(DefKind::Mod, next.to_def_id())
            {
                break;
            }

            base += 1;
            consumed += 1;
        }

        let Some((path, _)) = workspace.locate(cx, first.ident.span) else {
            return;
        };

        let mut source_segments = Vec::new();
        for segment in segments {
            let Some((segment_path, range)) = workspace.locate(cx, segment.ident.span) else {
                return;
            };

            if segment_path != path {
                return;
            }

            source_segments.push(Segment {
                name: segment.ident.name.to_string(),
                range,
            });
        }

        let Some(plan) = analysis::plan(&source_segments, depth, base, consumed) else {
            return;
        };

        self.facts.record(
            cx,
            workspace,
            first.ident.span,
            id,
            Import {
                segments: source_segments,
                plan,
                glob: matches!(tree.kind, hir::UseKind::Glob),
            },
        );
    }

    pub(crate) fn analyze(&self, path: &Path, source: &Source, workspace: &Workspace) -> RuleOutput {
        let imports: Vec<_> = self
            .facts
            .file(path, &workspace.modules)
            .map(|fact| &fact.value)
            .collect();

        analysis::analyze(source.parsed.syntax(), &source.text, &imports)
    }
}
