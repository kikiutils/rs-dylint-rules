use rustc_hir::{
    self as hir,
    def::{
        DefKind,
        Res,
    },
};
use rustc_lint::LateContext;

use super::{
    super::{
        model::Event,
        names::Segment,
    },
    Collector,
};
use crate::workspace::Workspace;

impl Collector {
    pub(super) fn import(
        &mut self,
        cx: &LateContext<'_>,
        workspace: &mut Workspace,
        tree: &hir::UseTree<'_>,
        id: hir::HirId,
    ) {
        self.import_tree(cx, workspace, tree, id, &[]);
    }

    fn import_tree(
        &mut self,
        cx: &LateContext<'_>,
        workspace: &mut Workspace,
        tree: &hir::UseTree<'_>,
        id: hir::HirId,
        parent: &[Segment],
    ) {
        let segments = self.names.import_path(tree, id, parent);

        match tree.kind {
            hir::UseKind::Nested { items } => {
                for (child, child_id, _) in items {
                    self.import_tree(cx, workspace, child, *child_id, &segments);
                }
            },
            hir::UseKind::Single(binding) => {
                for resolution in tree.prefix.res.iter().flatten() {
                    self.names.binding(cx, id, binding.name.as_str(), *resolution);
                }

                if binding.span.from_expansion() {
                    return;
                }

                let renamed = tree
                    .prefix
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident.span != binding.span);

                for resolution in tree.prefix.res.iter().flatten() {
                    let Res::Def(_, definition) = resolution else { continue };
                    if let Some(api) = self.api(cx, *definition) {
                        self.record(
                            cx,
                            workspace,
                            binding.span,
                            id,
                            Event::Import {
                                api,
                                name: binding.name.to_string(),
                                renamed,
                            },
                        );
                    }

                    if let Some(qualifier) = self.catalog(cx).qualifier(*definition).cloned() {
                        self.record(
                            cx,
                            workspace,
                            binding.span,
                            id,
                            Event::NamespaceImport {
                                qualifier,
                                name: binding.name.to_string(),
                                renamed,
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

                        let Res::Def(_, definition) = child.res else { continue };
                        if tree.prefix.span.from_expansion() {
                            continue;
                        }

                        if let Some(api) = self.api(cx, definition) {
                            self.record(
                                cx,
                                workspace,
                                tree.prefix.span,
                                id,
                                Event::Import {
                                    api,
                                    name: child.ident.name.to_string(),
                                    renamed: false,
                                },
                            );
                        }

                        if let Some(qualifier) = self.catalog(cx).qualifier(definition).cloned() {
                            self.record(
                                cx,
                                workspace,
                                tree.prefix.span,
                                id,
                                Event::NamespaceImport {
                                    qualifier,
                                    name: child.ident.name.to_string(),
                                    renamed: false,
                                },
                            );
                        }
                    }
                }
            },
        }
    }
}
