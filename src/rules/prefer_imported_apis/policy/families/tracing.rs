use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[
    Family::qualified(
        "tracing_subscriber",
        "tracing_subscriber",
        "tracing_subscriber",
        Members::Named(&["fmt"]),
    ),
    // Prefer the root re-export `tracing_subscriber::fmt()` to `fmt::fmt()`.
    Family::qualified(
        "tracing_subscriber::fmt",
        "tracing_subscriber",
        "tracing_subscriber",
        Members::Owned { constants: false },
    )
    .fallback(),
];
