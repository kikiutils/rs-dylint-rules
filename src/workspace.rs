use std::{
    cell::OnceCell,
    collections::{
        BTreeMap,
        BTreeSet,
    },
    sync::Arc,
};

use pathkit::{
    Path,
    path,
};
use ra_ap_syntax::TextRange;
use rustc_lint::{
    LateContext,
    LintContext,
};
use rustc_span::{
    BytePos,
    FileName,
    Span,
    def_id::LocalModId,
};

struct Module {
    id: LocalModId,
    range: TextRange,
}

// Module ownership excludes include! fragments. Equal ranges in distinct modules
// identify a shared physical source; distinct inline module ranges are safe.
#[derive(Default)]
pub(crate) struct ModuleSources {
    modules: BTreeMap<Path, Vec<Module>>,
    shared: BTreeSet<Path>,
}

impl ModuleSources {
    fn record(&mut self, path: Path, range: TextRange, module: LocalModId) {
        let entries = self.modules.entry(path.clone()).or_default();
        if entries.iter().any(|entry| entry.range == range && entry.id != module) {
            self.shared.insert(path);
        }

        entries.push(Module { id: module, range });
    }

    pub(crate) fn contains(&self, path: &Path, module: LocalModId) -> bool {
        self.modules
            .get(path)
            .is_some_and(|entries| entries.iter().any(|entry| entry.id == module))
    }

    pub(crate) fn contains_path(&self, path: &Path) -> bool {
        self.modules.contains_key(path)
    }

    pub(crate) fn can_fix(&self, path: &Path) -> bool {
        !self.shared.contains(path)
    }
}

// Crate-wide view of the workspace files seen during collection. The workspace
// root and every rustc source file lookup are canonicalized once and cached.
#[derive(Default)]
pub(crate) struct Workspace {
    root: OnceCell<Option<Path>>,
    paths: BTreeMap<BytePos, Option<Path>>,
    pub(crate) files: BTreeMap<Path, Span>,
    pub(crate) modules: ModuleSources,
}

impl Workspace {
    fn file(cx: &LateContext<'_>, span: Span) -> Option<Arc<rustc_span::SourceFile>> {
        if span.is_dummy() {
            return None;
        }

        let file = cx.sess().source_map().lookup_source_file(span.lo());
        file.contains(span.hi()).then_some(file)
    }

    fn range(file: &rustc_span::SourceFile, span: Span) -> TextRange {
        TextRange::new(
            file.original_relative_byte_pos(span.lo()).0.into(),
            file.original_relative_byte_pos(span.hi()).0.into(),
        )
    }

    pub(crate) fn locate(&mut self, cx: &LateContext<'_>, span: Span) -> Option<(Path, TextRange)> {
        let file = Self::file(cx, span)?;
        let root = self
            .root
            .get_or_init(|| path!(std::env::current_dir().ok()?).canonicalize().ok());

        let path = self
            .paths
            .entry(file.start_pos)
            .or_insert_with(|| {
                let FileName::Real(name) = &file.name else { return None };
                let path = path!(name.local_path()?).canonicalize().ok()?;
                path.starts_with(root.as_ref()?).then_some(path)
            })
            .as_ref()?
            .clone();

        self.files.entry(path.clone()).or_insert(span);
        Some((path, Self::range(&file, span)))
    }

    // Read-only lookup for files already located during collection.
    pub(crate) fn locate_known(&self, cx: &LateContext<'_>, span: Span) -> Option<(&Path, TextRange)> {
        let file = Self::file(cx, span)?;
        let path = self.paths.get(&file.start_pos)?.as_ref()?;
        Some((path, Self::range(&file, span)))
    }

    pub(crate) fn module(&mut self, cx: &LateContext<'_>, span: Span, module: LocalModId) {
        let Some((path, range)) = self.locate(cx, span) else {
            return;
        };

        self.modules.record(path, range, module);
    }
}
