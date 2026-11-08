use ra_ap_syntax::{
    TextRange,
    TextSize,
};
use rustc_hir::def::Namespace;
use rustc_span::def_id::{
    CRATE_DEF_INDEX,
    DefId,
    LOCAL_CRATE,
};

mod direct;
mod imports;
mod macro_inputs;
mod qualified;
mod safety;
mod transactions;

use super::super::{
    model::{
        Event,
        UseKind,
    },
    policy::{
        Api,
        Form,
        Qualifier,
    },
};
use crate::{
    diagnostic::RuleOutput,
    test_support::{
        apply,
        parse,
    },
};

fn definition() -> DefId {
    DefId {
        krate: LOCAL_CRATE,
        index: CRATE_DEF_INDEX,
    }
}

fn api(name: &str, namespace: Namespace, form: Form) -> Api {
    Api {
        definition: definition(),
        name: name.to_owned(),
        namespace,
        form,
    }
}

fn qualifier(source: &'static str, name: &'static str, external: bool) -> Qualifier {
    Qualifier {
        source,
        name,
        definition: definition(),
        root: definition(),
        external,
    }
}

fn range(source: &str, needle: &str) -> TextRange {
    ranges(source, needle)
        .next()
        .unwrap_or_else(|| panic!("missing {needle}"))
}

fn ranges<'a>(source: &'a str, needle: &'a str) -> impl Iterator<Item = TextRange> + 'a {
    source
        .match_indices(needle)
        .map(|(start, text)| TextRange::at(TextSize::of(&source[..start]), TextSize::of(text)))
}

fn analyze(source: &str, facts: &[(TextRange, Event)], free: bool) -> RuleOutput {
    super::analyze(
        &parse(source),
        source,
        ra_ap_syntax::Edition::Edition2024,
        facts
            .iter()
            .map(|(range, event)| (*range, rustc_hir::CRATE_HIR_ID, event)),
        &super::super::fix::Checks {
            name: &|_| free,
            remove: &|_| true,
            protected: &crate::syntax::protected,
            reference: &|_| false,
            depends: &|_| false,
        },
    )
}
