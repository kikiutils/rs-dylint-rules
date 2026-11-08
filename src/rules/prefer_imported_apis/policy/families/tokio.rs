use super::{
    Family,
    Members,
};

pub(super) const FAMILIES: &[Family] = &[
    Family::direct("tokio::time", Members::Functions(&[])),
    Family::qualified("tokio::task", "tokio", "tokio", Members::Functions(&[])),
    Family::qualified(
        "tokio::task",
        "tokio",
        "tokio",
        Members::Named(&["JoinHandle", "JoinError", "JoinSet"]),
    ),
    Family::qualified("tokio::process", "tokio", "tokio", Members::Owned { constants: false }),
    Family::qualified("tokio::sync", "tokio", "tokio", Members::Named(&["Mutex", "RwLock"])),
    Family::qualified("tokio::time", "tokio", "tokio", Members::Named(&["Instant"])),
    Family::qualified("tokio::sync::mpsc", "tokio", "tokio", Members::Channel),
    Family::qualified("tokio::sync::oneshot", "tokio", "tokio", Members::Channel),
    Family::qualified("tokio::sync::broadcast", "tokio", "tokio", Members::Channel),
    Family::qualified("tokio::sync::watch", "tokio", "tokio", Members::Channel),
];
