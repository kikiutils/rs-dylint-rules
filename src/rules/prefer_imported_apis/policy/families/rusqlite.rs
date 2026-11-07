use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[Family::qualified(
    "rusqlite",
    "rusqlite",
    "rusqlite",
    Members::Named(&["Error"]),
)];
