use std::collections::BTreeMap;

use pathkit::Path;
use ra_ap_syntax::{
    AstNode,
    SyntaxNode,
    TextRange,
    ast::{
        self,
        HasName,
    },
};
use rustc_hir::{
    self as hir,
    def::Res,
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
    def_id::LocalModId,
    kw,
};

use super::analysis::Group;

declare_lint! { pub KIKI_IMPORT_MODULE_ORDER, Warn, "order imports and external module declarations" }

pub(super) fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[KIKI_IMPORT_MODULE_ORDER]);
    store.register_late_lint_pass(Box::new(|_| Box::<ImportModuleOrder>::default()));
}

#[derive(Default)]
struct ImportModuleOrder {
    files: BTreeMap<(Path, u32), FileFacts>,
    sources: crate::source::ModuleSources,
}

struct FileFacts {
    anchor: Span,
    imports: BTreeMap<(u32, u32), (Option<Group>, hir::HirId)>,
    modules: Vec<(TextRange, hir::HirId)>,
}

impl FileFacts {
    fn active(&self, node: &SyntaxNode) -> bool {
        let range = node.text_range();
        if ast::Use::can_cast(node.kind()) {
            return self
                .imports
                .keys()
                .any(|&(start, end)| range.contains_range(text_range(start, end)));
        }
        if let Some(module) = ast::Module::cast(node.clone()) {
            return module
                .name()
                .is_some_and(|name| self.modules.iter().any(|(span, _)| *span == name.syntax().text_range()));
        }
        self.imports
            .keys()
            .any(|&(start, end)| range.contains_range(text_range(start, end)))
            || self.modules.iter().any(|(span, _)| range.contains_range(*span))
    }
}

impl ImportModuleOrder {
    fn file(&mut self, cx: &LateContext<'_>, span: Span, module: LocalModId) -> Option<(&mut FileFacts, TextRange)> {
        let (path, range) = crate::source::locate(cx, span)?;
        Some((
            self.files
                .entry((path, module.to_local_def_id().local_def_index.as_u32()))
                .or_insert_with(|| FileFacts {
                    anchor: span,
                    imports: BTreeMap::new(),
                    modules: Vec::new(),
                }),
            range,
        ))
    }

    fn import(&mut self, cx: &LateContext<'_>, tree: &hir::UseTree<'_>, id: hir::HirId) {
        if let Some(segment) = tree
            .prefix
            .segments
            .iter()
            .find(|segment| segment.ident.name != kw::PathRoot)
        {
            let group = match segment.res {
                Res::Def(_, definition) => Some(if definition.is_local() {
                    Group::Local
                } else {
                    Group::External
                }),
                _ if tree.prefix.segments.len() == 1 => {
                    let mut groups = tree.prefix.res.iter().filter_map(|resolution| match resolution {
                        Some(Res::Def(_, definition)) => Some(if definition.is_local() {
                            Group::Local
                        } else {
                            Group::External
                        }),
                        _ => None,
                    });
                    groups.next().filter(|group| groups.all(|other| other == *group))
                },
                _ => None,
            };
            if let Some((file, range)) = self.file(cx, segment.ident.span, cx.tcx.parent_module(id)) {
                let key = (range.start().into(), range.end().into());
                file.imports
                    .entry(key)
                    .and_modify(|(existing, _)| {
                        if *existing != group {
                            *existing = None;
                        }
                    })
                    .or_insert((group, id));
            }
        }
        if let hir::UseKind::Nested { items } = tree.kind {
            for (tree, hir_id, _) in items {
                self.import(cx, tree, *hir_id);
            }
        }
    }
}

impl_lint_pass!(ImportModuleOrder => [KIKI_IMPORT_MODULE_ORDER]);

impl<'tcx> LateLintPass<'tcx> for ImportModuleOrder {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx hir::Mod<'tcx>, id: LocalModId) {
        self.sources.record(
            cx,
            module.spans.inner_span,
            id.to_local_def_id().local_def_index.as_u32(),
        );
        self.file(cx, module.spans.inner_span, id);
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        match item.kind {
            hir::ItemKind::Use(tree) => self.import(cx, &tree, item.hir_id()),
            hir::ItemKind::Mod(name, _) => {
                if let Some((file, range)) = self.file(cx, name.span, cx.tcx.parent_module(item.hir_id())) {
                    file.modules.push((range, item.hir_id()));
                }
            },
            _ => {},
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for ((path, module), facts) in &self.files {
            if !self.sources.contains(path, *module) {
                continue;
            }
            let source = match crate::source::Source::load(cx, path, facts.anchor) {
                Ok(source) => source,
                Err(error) => {
                    cx.sess().dcx().span_err(facts.anchor, format!("{error:#}"));
                    continue;
                },
            };
            let output = super::analysis::analyze(
                source.parsed.syntax(),
                &|path| {
                    let reference = path.segment()?.name_ref()?;
                    let range = reference.syntax().text_range();
                    facts
                        .imports
                        .get(&(range.start().into(), range.end().into()))
                        .and_then(|(group, _)| *group)
                },
                &|node| facts.active(node),
            );
            emit_diagnostics(cx, facts, &source, &output, self.sources.can_fix(path));
        }
    }
}

fn text_range(start: u32, end: u32) -> TextRange {
    TextRange::new(start.into(), end.into())
}

fn emit_diagnostics(
    cx: &LateContext<'_>,
    facts: &FileFacts,
    source: &crate::source::Source,
    output: &crate::diagnostic::RuleOutput,
    can_fix: bool,
) {
    for issue in &output.issues {
        let diagnostic = &issue.diagnostics[0];
        let id = facts
            .imports
            .iter()
            .find_map(|(&(start, end), (_, id))| diagnostic.range.contains_range(text_range(start, end)).then_some(*id))
            .or_else(|| {
                facts
                    .modules
                    .iter()
                    .find_map(|(range, id)| diagnostic.range.contains_range(*range).then_some(*id))
            })
            .unwrap_or(hir::CRATE_HIR_ID);
        issue.emit(
            cx,
            KIKI_IMPORT_MODULE_ORDER,
            id,
            source,
            "reorder import/module sections",
            can_fix,
        );
    }
}
