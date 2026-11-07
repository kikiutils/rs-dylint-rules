mod anyhow;
mod confique;
mod formats;
mod rusqlite;
mod standard;
mod tokio;

pub(super) struct Family {
    pub(super) module: &'static str,
    pub(super) qualifier: Option<(&'static str, &'static str)>,
    pub(super) members: Members,
}

pub(super) enum Members {
    Functions(&'static [&'static str]),
    Named(&'static [&'static str]),
    Channel,
}

impl Family {
    const fn direct(module: &'static str, members: Members) -> Self {
        Self {
            module,
            qualifier: None,
            members,
        }
    }

    const fn qualified(module: &'static str, source: &'static str, name: &'static str, members: Members) -> Self {
        Self {
            module,
            qualifier: Some((source, name)),
            members,
        }
    }
}

pub(super) fn all() -> impl Iterator<Item = &'static Family> {
    standard::FAMILIES
        .iter()
        .chain(anyhow::FAMILIES)
        .chain(formats::FAMILIES)
        .chain(rusqlite::FAMILIES)
        .chain(confique::FAMILIES)
        .chain(tokio::FAMILIES)
}
