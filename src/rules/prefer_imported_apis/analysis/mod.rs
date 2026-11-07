use kikiutils::types::fx_collections::FxHashSet;
use ra_ap_syntax::{
    Edition,
    SyntaxNode,
    TextRange,
};

mod accesses;
mod imports;
mod paths;

pub(super) use self::{
    accesses::{
        Access,
        visible_scopes,
    },
    imports::Import,
};
use super::{
    fix,
    model::{
        Event,
        UseKind,
    },
    names::NameRequest,
};
use crate::diagnostic::RuleOutput;

pub(super) fn analyze<'a>(
    root: &SyntaxNode,
    source: &str,
    edition: Edition,
    facts: impl IntoIterator<Item = (TextRange, &'a Event)>,
    safe_name: impl Fn(NameRequest<'_>) -> bool,
) -> RuleOutput {
    let mut output = RuleOutput::default();
    let mut imports = Vec::new();
    let mut accesses = Vec::new();
    let mut reported_imports = FxHashSet::default();
    let mut seen = FxHashSet::default();
    for (range, event) in facts.into_iter().filter(|entry| seen.insert(*entry)) {
        match event {
            Event::Import { .. } | Event::NamespaceImport { .. } => {
                if let Some(import) = imports::read(root, range, event, &mut reported_imports, &mut output) {
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
        if access.kind == UseKind::Trait || access.is_canonical() {
            accesses::check_import_order(access, &imports, &mut reported_imports, &mut output);
        } else {
            let issue = output.issues.len();
            output.report(access.diagnostic());
            requests.push((issue, access));
        }
    }

    fix::plan(root, source, &imports, &requests, &safe_name, &mut output);
    output
}

pub(super) fn scope(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.ancestors().find(crate::syntax::is_scope)
}

#[cfg(test)]
mod tests;
