use ra_ap_syntax::TextRange;
use rustc_span::def_id::DefId;

use super::policy::{
    Api,
    Qualifier,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct FactKey {
    pub(super) range: TextRange,
    pub(super) definition: DefId,
}

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

impl Event {
    pub(super) fn definition(&self) -> DefId {
        match self {
            Self::Use { api, .. } | Self::Import { api, .. } => api.definition,
            Self::NamespaceImport { qualifier, .. } => qualifier.definition,
        }
    }
}
