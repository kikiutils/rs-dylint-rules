use std::collections::BTreeSet;

use rustc_hir::def::Namespace;
use rustc_span::def_id::{
    CRATE_DEF_INDEX,
    DefId,
    LOCAL_CRATE,
};

use super::{
    Api,
    Form,
    ImportTarget,
    Qualifier,
    families::{
        self,
        Members,
    },
};

fn definition() -> DefId {
    DefId {
        krate: LOCAL_CRATE,
        index: CRATE_DEF_INDEX,
    }
}

#[test]
fn canonical_forms_keep_the_namespace_and_member_independent() {
    let mut api = Api {
        definition: definition(),
        name: "TryRecvError".to_owned(),
        source: "tokio::sync::mpsc::error::TryRecvError".to_owned(),
        root: Some(definition()),
        namespace: Namespace::TypeNS,
        form: Form::Direct,
    };

    assert_eq!(api.canonical(), "TryRecvError");
    assert_eq!(
        api.full_path().as_deref(),
        Some("tokio::sync::mpsc::error::TryRecvError")
    );

    api.form = Form::Qualified {
        qualifier: Qualifier {
            source: "tokio::sync::mpsc",
            name: "tokio_mpsc",
            definition: definition(),
            root: definition(),
            external: false,
        },
        member: "error::TryRecvError".to_owned(),
    };

    assert_eq!(api.canonical(), "tokio_mpsc::error::TryRecvError");
}

#[test]
fn replacement_full_path_requires_an_available_target() {
    let mut api = Api {
        definition: definition(),
        name: "Result".to_owned(),
        source: "anyhow::Result".to_owned(),
        root: None,
        namespace: Namespace::TypeNS,
        form: Form::Replace { target: None },
    };

    assert_eq!(api.full_path(), None);
    api.form = Form::Replace {
        target: Some(ImportTarget {
            source: "anyhow::Result",
            definition: None,
            root: None,
        }),
    };

    assert_eq!(api.full_path().as_deref(), Some("anyhow::Result"));
}

#[test]
fn family_routes_are_consistent_and_explicit_members_are_unique() {
    for family in families::all() {
        if let Some((source, name)) = family.qualifier {
            assert!(family.module == source || family.module.starts_with(&format!("{source}::")));
            assert_ne!(name, "");
        }

        if let Members::Named(names) = family.members {
            assert_eq!(
                names.len(),
                names.iter().collect::<BTreeSet<_>>().len(),
                "{}",
                family.module
            );
        }
    }
}

#[test]
fn prelude_functions_are_excluded_without_excluding_ordinary_memory_functions() {
    for family in families::all().filter(|family| matches!(family.module, "std::mem" | "core::mem")) {
        let Members::Functions(excluded) = family.members else {
            panic!("memory function family expected")
        };

        assert_eq!(excluded, ["drop", "size_of", "size_of_val", "align_of", "align_of_val"]);
    }
}
