use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[
    Family::direct(
        "anyhow",
        Members::Named(&["Context", "Result", "anyhow", "bail", "ensure"]),
    ),
    Family::qualified("anyhow", "anyhow", "anyhow", Members::Named(&["Error"])),
];
