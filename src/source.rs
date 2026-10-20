use std::sync::Arc;

use anyhow::{
    Context,
    Result,
    anyhow,
    ensure,
};
use pathkit::{
    Path,
    SyncFsOps,
};
use ra_ap_syntax::{
    Edition,
    SourceFile,
    TextRange,
};
use rustc_lint::{
    LateContext,
    LintContext,
};
use rustc_span::Span;

pub(crate) struct Source {
    pub(crate) parsed: SourceFile,
    pub(crate) text: String,
    file: Arc<rustc_span::SourceFile>,
}

impl Source {
    pub(crate) fn load(cx: &LateContext<'_>, path: &Path, anchor: Span) -> Option<Self> {
        match Self::read(cx, path, anchor) {
            Ok(source) => Some(source),
            Err(error) => {
                cx.sess().dcx().span_warn(anchor, format!("{error:#}"));
                None
            },
        }
    }

    fn read(cx: &LateContext<'_>, path: &Path, anchor: Span) -> Result<Self> {
        let text = path
            .read_to_string_sync()
            .with_context(|| format!("Failed to read {path}"))?;

        let file = cx.sess().source_map().lookup_source_file(anchor.lo());
        ensure!(
            file.src
                .as_ref()
                .is_some_and(|source| source.as_str() == normalized(&text)),
            "Source changed since rustc loaded {path}"
        );

        let edition = cx
            .sess()
            .edition()
            .to_string()
            .parse()
            .map_err(|error| anyhow!("Unsupported Rust edition: {error}"))?;

        let parsed =
            parse(&text, edition).with_context(|| format!("Cannot parse {path} as a Rust item source file"))?;

        Ok(Self { parsed, text, file })
    }

    pub(crate) fn span(&self, range: TextRange) -> Span {
        Span::with_root_ctxt(
            self.file.normalized_byte_pos(range.start().into()),
            self.file.normalized_byte_pos(range.end().into()),
        )
    }
}

fn normalized(text: &str) -> String {
    text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n")
}

fn parse(text: &str, edition: Edition) -> Result<SourceFile> {
    // Preserve original byte offsets, including BOM and CRLF, for rustc spans.
    let padded;
    let input = if let Some(rest) = text.strip_prefix('\u{feff}') {
        padded = format!("   {rest}");
        &padded
    } else {
        text
    };

    let parsed = SourceFile::parse(input, edition);
    ensure!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    Ok(parsed.tree())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use ra_ap_syntax::{
        AstNode,
        Edition,
        ast,
    };

    #[test]
    fn bom_and_crlf_preserve_original_offsets() -> Result<()> {
        for newline in ["\n", "\r\n"] {
            let text = format!("\u{feff}// 型別{newline}fn first() {{}}{newline}fn second() {{}}{newline}");
            let parsed = super::parse(&text, Edition::Edition2024)?;
            let functions: Vec<_> = parsed.syntax().children().filter_map(ast::Fn::cast).collect();
            assert_eq!(functions.len(), 2);
            for function in functions {
                let Some(keyword) = function.fn_token() else {
                    panic!("missing fn keyword")
                };

                assert_eq!(&text[keyword.text_range()], "fn");
            }

            assert!(!super::normalized(&text).contains('\r'));
            assert!(!super::normalized(&text).starts_with('\u{feff}'));
        }

        Ok(())
    }

    #[test]
    fn parser_failure_is_recoverable() {
        assert!(super::parse("fn {", Edition::Edition2024).is_err());
    }
}
