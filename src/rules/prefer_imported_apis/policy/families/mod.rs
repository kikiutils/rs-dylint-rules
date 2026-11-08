mod anyhow;
mod confique;
mod formats;
mod platform;
mod rusqlite;
mod standard;
mod tokio;
mod tracing;

pub(super) struct Family {
    pub(super) module: &'static str,
    pub(super) qualifier: Option<(&'static str, &'static str)>,
    pub(super) members: Members,
    pub(super) priority: u8,
}

pub(super) enum Members {
    Functions(&'static [&'static str]),
    Named(&'static [&'static str]),
    Channel,
    Owned { constants: bool },
}

impl Family {
    const fn direct(module: &'static str, members: Members) -> Self {
        Self {
            module,
            qualifier: None,
            members,
            priority: 0,
        }
    }

    const fn qualified(module: &'static str, source: &'static str, name: &'static str, members: Members) -> Self {
        Self {
            module,
            qualifier: Some((source, name)),
            members,
            priority: 0,
        }
    }

    const fn fallback(mut self) -> Self {
        self.priority = 1;
        self
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
        .chain(tracing::FAMILIES)
        .chain(platform::FAMILIES)
}
