use kikiutils::types::fx_collections::FxHashSet;
use ra_ap_syntax::{
    Edition,
    SyntaxNode,
    TextRange,
};

mod accesses;
pub(super) mod attributes;
mod decision;
mod declarations;
mod paths;
mod targets;

pub(super) use self::{
    accesses::Access,
    decision::Decision,
};
use super::{
    fix,
    model::Event,
    names::NameRequest,
};
use crate::diagnostic::RuleOutput;

pub(super) struct Checks<'a> {
    pub(super) name: &'a dyn Fn(NameRequest<'_>) -> bool,
    pub(super) protected: &'a dyn Fn(&SyntaxNode) -> bool,
}

pub(super) fn analyze<'a>(
    root: &SyntaxNode,
    source: &str,
    edition: Edition,
    facts: impl IntoIterator<Item = (TextRange, &'a Event)>,
    checks: &Checks<'_>,
) -> RuleOutput {
    let mut seen = FxHashSet::default();
    let mut accesses = Vec::new();
    let mut imports = Vec::new();
    for (range, event) in facts {
        if let Some(import) = declarations::ExistingImport::read(root, range, event) {
            imports.push(import);
        }

        if let Event::Use { api, kind } = event
            && seen.insert((range, event))
            && let Some(access) = accesses::read(root, range, api.clone(), *kind, edition)
        {
            accesses.push(access);
        }
    }

    accesses.sort_by_key(|access| access.range.start());
    let covered = decision::covered(&accesses, &imports, checks);
    let decisions: Vec<_> = accesses
        .iter()
        .map(|access| decision::decide(access, source, &covered, &imports, checks))
        .collect();

    let mut output = RuleOutput::default();
    fix::emit(&accesses, &decisions, &mut output);
    output
}

#[cfg(test)]
mod tests;
