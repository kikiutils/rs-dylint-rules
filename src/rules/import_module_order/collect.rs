use kikiutils::types::fx_collections::FxHashSet;
use pathkit::Path;
use ra_ap_syntax::AstNode;
use rustc_hir::{
    self as hir,
    def::Res,
};
use rustc_lint::LateContext;
use rustc_span::kw;

use super::analysis::{
    self,
    Group,
};
use crate::{
    diagnostic::RuleOutput,
    facts::Facts,
    source::Source,
    workspace::Workspace,
};

fn origin(resolution: Res) -> Option<Group> {
    let Res::Def(_, definition) = resolution else {
        return None;
    };

    Some(if definition.is_local() {
        Group::Local
    } else {
        Group::External
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Import(Option<Group>),
    Module,
}

#[derive(Default)]
pub(crate) struct Collector {
    facts: Facts<Kind>,
}

impl Collector {
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, workspace: &mut Workspace, item: &hir::Item<'_>) {
        match item.kind {
            hir::ItemKind::Use(tree) => self.import(cx, workspace, &tree, item.hir_id()),
            hir::ItemKind::Mod(name, _) => self.facts.record(cx, workspace, name.span, item.hir_id(), Kind::Module),
            _ => {},
        }
    }

    fn import(&mut self, cx: &LateContext<'_>, workspace: &mut Workspace, tree: &hir::UseTree<'_>, id: hir::HirId) {
        if let Some(segment) = tree
            .prefix
            .segments
            .iter()
            .find(|segment| segment.ident.name != kw::PathRoot)
        {
            let group = match origin(segment.res) {
                Some(group) => Some(group),
                // A single-segment import may resolve in several namespaces.
                None if tree.prefix.segments.len() == 1 => {
                    let mut groups = tree
                        .prefix
                        .res
                        .iter()
                        .filter_map(|resolution| resolution.and_then(origin));

                    groups.next().filter(|group| groups.all(|other| other == *group))
                },
                None => None,
            };

            self.facts
                .record(cx, workspace, segment.ident.span, id, Kind::Import(group));
        }

        if let hir::UseKind::Nested { items } = tree.kind {
            for (tree, hir_id, _) in items {
                self.import(cx, workspace, tree, *hir_id);
            }
        }
    }

    pub(crate) fn analyze(&self, path: &Path, source: &Source, workspace: &Workspace) -> RuleOutput {
        let root = source.parsed.syntax();
        let imports = self.facts.resolved(path, &workspace.modules);
        let active: FxHashSet<_> = self
            .facts
            .file(path, &workspace.modules)
            .flat_map(|fact| root.covering_element(fact.range).ancestors())
            .collect();

        analysis::analyze(
            root,
            &|path| {
                let range = path.segment()?.name_ref()?.syntax().text_range();
                match imports.get(&range) {
                    Some(Some(Kind::Import(group))) => *group,
                    _ => None,
                }
            },
            &|node| active.contains(node),
        )
    }
}
