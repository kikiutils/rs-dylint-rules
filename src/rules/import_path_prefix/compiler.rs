use std::collections::BTreeMap;

use pathkit::Path;
use ra_ap_syntax::{
    AstNode,
    ast,
};
use rustc_errors::{
    Applicability,
    DiagDecorator,
};
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
    LintContext,
};
use rustc_lint_defs::{
    declare_lint,
    impl_lint_pass,
};
use rustc_span::{
    Span,
    kw,
};

use super::analysis::{
    self,
    Plan,
    Segment,
};

declare_lint! { pub KIKI_IMPORT_PATH_PREFIX, Warn, "use module-relative prefixes for imports within the current crate" }

pub(super) fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[KIKI_IMPORT_PATH_PREFIX]);
    store.register_late_lint_pass(Box::new(|_| Box::<ImportPathPrefix>::default()));
}

#[derive(Default)]
struct ImportPathPrefix {
    files: BTreeMap<(Path, u32), FileFacts>,
}

struct FileFacts {
    anchor: Span,
    paths: Vec<Import>,
}

struct Import {
    id: hir::HirId,
    segments: Vec<Segment>,
    plan: Plan,
    glob: bool,
}

impl_lint_pass!(ImportPathPrefix => [KIKI_IMPORT_PATH_PREFIX]);

impl ImportPathPrefix {
    fn import<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        tree: &hir::UseTree<'tcx>,
        id: hir::HirId,
        parent: &[&hir::PathSegment<'tcx>],
    ) {
        let mut segments = parent.to_vec();
        segments.extend(tree.prefix.segments);
        if let hir::UseKind::Nested { items } = tree.kind {
            for (child, id, _) in items {
                self.import(cx, child, *id, &segments);
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
        let Some((path, _)) = crate::source::locate(cx, first.ident.span) else {
            return;
        };
        let mut source_segments = Vec::new();
        for segment in segments {
            let Some((segment_path, range)) = crate::source::locate(cx, segment.ident.span) else {
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
        self.files
            .entry((path, module.local_def_index.as_u32()))
            .or_insert_with(|| FileFacts {
                anchor: first.ident.span,
                paths: Vec::new(),
            })
            .paths
            .push(Import {
                id,
                segments: source_segments,
                plan,
                glob: matches!(tree.kind, hir::UseKind::Glob),
            });
    }
}

impl<'tcx> LateLintPass<'tcx> for ImportPathPrefix {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Use(tree) = item.kind {
            self.import(cx, &tree, item.hir_id(), &[]);
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for ((path, _), facts) in &self.files {
            let source = match crate::source::Source::load(cx, path, facts.anchor) {
                Ok(source) => source,
                Err(error) => {
                    cx.sess().dcx().span_err(facts.anchor, format!("{error:#}"));
                    continue;
                },
            };
            let mut emitted = Vec::new();
            let mut split_ranges = Vec::new();
            for import in &facts.paths {
                // Macro inputs are token trees, not source use items. Expansion
                // tokens can retain original spans even without expansion ctxt.
                if !source
                    .parsed
                    .syntax()
                    .descendants()
                    .filter_map(ast::Use::cast)
                    .any(|item| item.syntax().text_range().contains_range(import.segments[0].range))
                {
                    continue;
                }
                if emitted.contains(&import.plan) {
                    continue;
                }
                emitted.push(import.plan.clone());
                let edit = analysis::edit(source.parsed.syntax(), &import.segments, &import.plan, import.glob).filter(
                    |edit| {
                        facts
                            .paths
                            .iter()
                            .all(|other| other.plan == import.plan || other.plan.range.intersect(edit.range).is_none())
                    },
                );
                let edit = edit.or_else(|| {
                    let split = analysis::split_import(
                        source.parsed.syntax(),
                        &import.segments,
                        &facts
                            .paths
                            .iter()
                            .map(|path| (path.segments.as_slice(), &path.plan))
                            .collect::<Vec<_>>(),
                    )?;
                    if split_ranges.contains(&split.range) {
                        return None;
                    }
                    split_ranges.push(split.range);
                    Some(split)
                });
                cx.tcx.emit_node_span_lint(
                    KIKI_IMPORT_PATH_PREFIX,
                    import.id,
                    source.span(import.plan.range),
                    DiagDecorator(|diag| {
                        diag.primary_message(format!("expected import prefix `{}`", import.plan.replacement));
                        if let Some(edit) = edit {
                            diag.span_suggestion(
                                source.span(edit.range),
                                "use the module-relative import prefix",
                                edit.replacement,
                                Applicability::MachineApplicable,
                            );
                        }
                    }),
                );
            }
        }
    }
}
