use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[
    Family::qualified(
        "confique::toml",
        "confique",
        "confique",
        Members::Named(&["template", "FormatOptions"]),
    ),
    Family::qualified(
        "confique::yaml",
        "confique",
        "confique",
        Members::Named(&["template", "FormatOptions"]),
    ),
    Family::qualified(
        "confique::json5",
        "confique",
        "confique",
        Members::Named(&["template", "FormatOptions"]),
    ),
];
