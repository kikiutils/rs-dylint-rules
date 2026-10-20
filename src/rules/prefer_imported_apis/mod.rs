use rustc_hir::def::DefKind;
use rustc_lint::LateContext;
use rustc_span::def_id::{
    CRATE_DEF_INDEX,
    DefId,
};

mod analysis;
pub(super) mod collect;
mod resolution;

rustc_lint_defs::declare_lint! { pub KIKI_PREFER_IMPORTED_APIS, Warn, "import selected APIs using their prescribed names" }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Api {
    Crate,
    Anyhow,
    Bail,
    Ensure,
    Result,
    Error,
}

impl Api {
    fn name(self) -> &'static str {
        match self {
            Self::Crate | Self::Anyhow => "anyhow",
            Self::Bail => "bail",
            Self::Ensure => "ensure",
            Self::Result => "Result",
            Self::Error => "AnyhowError",
        }
    }

    fn allows(self, name: &str) -> bool {
        name == self.name() || (self == Self::Result && name == "AnyhowResult")
    }
}

fn api(cx: &LateContext<'_>, definition: DefId) -> Option<Api> {
    if cx.tcx.crate_name(definition.krate).as_str() != "anyhow" {
        return None;
    }

    if definition.index == CRATE_DEF_INDEX {
        return Some(Api::Crate);
    }

    if matches!(cx.tcx.def_kind(definition), DefKind::Macro(_)) {
        return match cx.tcx.item_name(definition).as_str() {
            "anyhow" => Some(Api::Anyhow),
            "bail" => Some(Api::Bail),
            "ensure" => Some(Api::Ensure),
            _ => None,
        };
    }

    if cx
        .tcx
        .opt_parent(definition)
        .is_none_or(|parent| parent.index != CRATE_DEF_INDEX)
    {
        return None;
    }

    match cx.tcx.item_name(definition).as_str() {
        "Result" => Some(Api::Result),
        "Error" => Some(Api::Error),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
