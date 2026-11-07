use super::policy::{
    Api,
    Qualifier,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum UseKind {
    Path,
    Macro,
    Trait,
}

// Source occurrences retain their compiler identity in Facts<Event>.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Event {
    Use {
        api: Api,
        kind: UseKind,
    },
    Import {
        api: Api,
        name: String,
        renamed: bool,
    },
    NamespaceImport {
        qualifier: Qualifier,
        name: String,
        renamed: bool,
    },
}
