use kikiutils::types::fx_collections::FxHashSet;
use ra_ap_syntax::{
    Edition,
    SyntaxNode,
    TextRange,
};

mod accesses;
pub(super) mod attributes;
pub(super) mod bindings;
mod fallback;
mod imports;
mod paths;

pub(super) use self::{
    accesses::Access,
    imports::Import,
};
use super::{
    fix,
    model::{
        Event,
        UseKind,
    },
};
use crate::diagnostic::RuleOutput;

pub(super) fn analyze<'a>(
    root: &SyntaxNode,
    source: &str,
    edition: Edition,
    facts: impl IntoIterator<Item = (TextRange, rustc_hir::HirId, &'a Event)>,
    checks: &fix::Checks<'_>,
) -> RuleOutput {
    let mut output = RuleOutput::default();
    let mut imports = Vec::new();
    let mut accesses = Vec::new();
    let mut reported_imports = FxHashSet::default();
    let mut seen = FxHashSet::default();
    for (range, origin, event) in facts
        .into_iter()
        .filter(|(range, _, event)| seen.insert((*range, *event)))
    {
        match event {
            Event::Import { .. } | Event::NamespaceImport { .. } => {
                if let Some(import) = imports::read(root, range, event, origin, &mut reported_imports, &mut output) {
                    imports.push(import);
                }
            },
            Event::Use { api, kind } => {
                if let Some(access) = accesses::read(root, range, api.clone(), *kind, edition) {
                    accesses.push(access);
                }
            },
        }
    }

    accesses.sort_by_key(|access| (access.scope.text_range().start(), access.range.start()));
    let mut requests = Vec::new();
    for access in &accesses {
        if access.kind == UseKind::Trait
            || access.is_canonical()
            || fallback::is_fallback(access, &imports, checks.name)
        {
            accesses::check_import_order(access, &imports, &mut reported_imports, &mut output);
        } else {
            output.report(access.diagnostic());
            requests.push(access);
        }
    }

    fix::plan(root, source, &imports, &accesses, &requests, checks, &mut output);

    output
}

#[cfg(test)]
mod tests;
