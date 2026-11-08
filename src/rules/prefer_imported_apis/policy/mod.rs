use rustc_hir::def::Namespace;
use rustc_span::def_id::DefId;

mod catalog;
mod families;

pub(super) use self::catalog::Catalog;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Qualifier {
    pub(super) source: &'static str,
    pub(super) name: &'static str,
    pub(super) definition: DefId,
    pub(super) root: DefId,
    pub(super) external: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Form {
    Direct,
    Qualified { qualifier: Qualifier, member: String },
    Replace { target: Option<ImportTarget> },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ImportTarget {
    pub(super) source: &'static str,
    pub(super) definition: Option<DefId>,
    pub(super) root: Option<DefId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Api {
    pub(super) definition: DefId,
    pub(super) name: String,
    pub(super) namespace: Namespace,
    pub(super) form: Form,
}

impl Api {
    pub(super) fn is_direct(&self) -> bool {
        matches!(self.form, Form::Direct | Form::Replace { .. })
    }

    pub(super) fn replaces_type(&self) -> bool {
        matches!(self.form, Form::Replace { .. })
    }

    pub(super) fn full_path(&self) -> Option<String> {
        let Form::Qualified { qualifier, member } = &self.form else {
            return None;
        };

        Some(format!("{}::{member}", qualifier.source))
    }

    pub(super) fn canonical(&self) -> String {
        match &self.form {
            Form::Direct | Form::Replace { .. } => self.name.clone(),
            Form::Qualified { qualifier, member } => format!("{}::{member}", qualifier.name),
        }
    }
}

#[cfg(test)]
mod tests;
