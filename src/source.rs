use std::sync::Arc;

use anyhow::{
    Context,
    Result,
    ensure,
};
use pathkit::{
    Path,
    SyncFsOps,
    path,
};
use ra_ap_syntax::{
    SourceFile,
    TextRange,
};
use rustc_lint::{
    LateContext,
    LintContext,
};
use rustc_span::{
    FileName,
    Span,
};

pub(crate) struct Source {
    pub(crate) parsed: SourceFile,
    file: Arc<rustc_span::SourceFile>,
}

pub(crate) fn locate(cx: &LateContext<'_>, span: Span) -> Option<(Path, TextRange)> {
    if span.is_dummy() {
        return None;
    }
    let file = cx.sess().source_map().lookup_source_file(span.lo());
    if !file.contains(span.hi()) {
        return None;
    }
    let FileName::Real(name) = &file.name else {
        return None;
    };
    let path = path!(name.local_path()?).canonicalize().ok()?;
    let root = path!(std::env::current_dir().ok()?).canonicalize().ok()?;
    if !path.starts_with(root) {
        return None;
    }
    let range = TextRange::new(
        file.original_relative_byte_pos(span.lo()).0.into(),
        file.original_relative_byte_pos(span.hi()).0.into(),
    );
    Some((path, range))
}

impl Source {
    pub(crate) fn load(cx: &LateContext<'_>, path: &Path, anchor: Span) -> Result<Self> {
        let text = path
            .read_to_string_sync()
            .with_context(|| format!("Failed to read {path}"))?;
        let file = cx.sess().source_map().lookup_source_file(anchor.lo());
        let normalized = text.strip_prefix('\u{feff}').unwrap_or(&text).replace("\r\n", "\n");
        ensure!(
            file.src.as_ref().is_some_and(|source| source.as_str() == normalized),
            "Source changed since rustc loaded {path}"
        );
        let edition = cx
            .sess()
            .edition()
            .to_string()
            .parse()
            .map_err(|error| anyhow::anyhow!("Unsupported Rust edition: {error}"))?;
        let parsed = SourceFile::parse(&text, edition);
        ensure!(
            parsed.errors().is_empty(),
            "Cannot parse {path} as a Rust item source file"
        );
        Ok(Self {
            parsed: parsed.tree(),
            file,
        })
    }

    pub(crate) fn span(&self, range: TextRange) -> Span {
        Span::with_root_ctxt(
            self.file.normalized_byte_pos(range.start().into()),
            self.file.normalized_byte_pos(range.end().into()),
        )
    }
}
