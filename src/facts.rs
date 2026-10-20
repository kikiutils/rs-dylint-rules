use std::collections::BTreeMap;

use kikiutils::types::fx_collections::FxHashMap;
use pathkit::Path;
use ra_ap_syntax::TextRange;
use rustc_hir::HirId;
use rustc_lint::LateContext;
use rustc_span::{
    Span,
    def_id::LocalModId,
};

use crate::workspace::{
    ModuleSources,
    Workspace,
};

pub(crate) struct Fact<T> {
    pub(crate) range: TextRange,
    pub(crate) id: HirId,
    pub(crate) module: LocalModId,
    pub(crate) value: T,
}

// Compiler facts grouped by physical file; each keeps its owning module.
pub(crate) struct Facts<T> {
    files: BTreeMap<Path, Vec<Fact<T>>>,
}

impl<T> Default for Facts<T> {
    fn default() -> Self {
        Self { files: BTreeMap::new() }
    }
}

impl<T> Facts<T> {
    pub(crate) fn record(&mut self, cx: &LateContext<'_>, workspace: &mut Workspace, span: Span, id: HirId, value: T) {
        let Some((path, range)) = workspace.locate(cx, span) else {
            return;
        };

        self.files.entry(path).or_default().push(Fact {
            range,
            id,
            module: cx.tcx.parent_module(id),
            value,
        });
    }

    // One value per range, or None when facts recorded at the same range disagree.
    pub(crate) fn resolved(&self, path: &Path, modules: &ModuleSources) -> FxHashMap<TextRange, Option<T>>
    where
        T: Copy + PartialEq,
    {
        let mut values = FxHashMap::default();
        for fact in self.file(path, modules) {
            values
                .entry(fact.range)
                .and_modify(|value: &mut Option<T>| {
                    if *value != Some(fact.value) {
                        *value = None;
                    }
                })
                .or_insert(Some(fact.value));
        }

        values
    }

    pub(crate) fn file<'a>(&'a self, path: &'a Path, modules: &'a ModuleSources) -> impl Iterator<Item = &'a Fact<T>> {
        self.files
            .get(path)
            .into_iter()
            .flatten()
            .filter(move |fact| modules.contains(path, fact.module))
    }
}
