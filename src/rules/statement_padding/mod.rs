use ra_ap_syntax::AstNode;

mod analysis;

use crate::{
    diagnostic::RuleOutput,
    source::Source,
};

rustc_lint_defs::declare_lint! { pub KIKI_STATEMENT_PADDING, Warn, "separate multi-line statements and adjacent items with blank lines" }

// Purely syntactic: the whole module source, including cfg-disabled code.
pub(super) fn analyze(source: &Source) -> RuleOutput {
    analysis::analyze(source.parsed.syntax(), &source.text)
}

#[cfg(test)]
mod tests;
