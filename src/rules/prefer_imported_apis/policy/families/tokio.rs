use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[
    Family::direct("tokio::time", Members::Functions(&[])),
    Family::qualified("tokio::task", "tokio::task", "tokio_task", Members::Functions(&[])),
    Family::qualified("tokio::sync::mpsc", "tokio::sync::mpsc", "tokio_mpsc", Members::Channel),
    Family::qualified(
        "tokio::sync::oneshot",
        "tokio::sync::oneshot",
        "tokio_oneshot",
        Members::Channel,
    ),
];
