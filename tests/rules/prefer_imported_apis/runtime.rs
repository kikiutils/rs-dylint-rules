use anyhow::Result;

use super::support;

#[test]
fn imports_time_functions_even_when_the_future_is_not_awaited_at_the_call_site() -> Result<()> {
    let source = r"use std::time::Duration;
pub async fn timers() {
    let delay = tokio::time::sleep(Duration::from_millis(1));
    let _ = tokio::time::timeout(Duration::from_secs(1), delay).await;
    let mut timer = tokio::time::interval(Duration::from_secs(1));
    timer.tick().await;
}
";

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(
        fixed,
        format!(
            "use tokio::time::sleep;\nuse tokio::time::timeout;\nuse tokio::time::interval;\n{}",
            source.replace("tokio::time::", "")
        )
    );

    Ok(())
}

#[test]
fn qualifies_task_functions_through_the_tokio_namespace() -> Result<()> {
    let source = r"pub async fn tasks() {
    let handle = tokio::spawn(async { 1 });
    let blocking = tokio::task::spawn_blocking(|| 2);
    tokio::task::yield_now().await;
    let _ = (handle.await, blocking.await);
}
";

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(fixed, source.replace("tokio::spawn", "tokio::task::spawn"));

    Ok(())
}

#[test]
fn accepts_channel_functions_types_and_errors_with_full_paths() -> Result<()> {
    let source = r"pub fn channels() -> (
    std::sync::mpsc::Sender<u8>,
    tokio::sync::mpsc::Sender<u8>,
    tokio::sync::oneshot::Sender<u8>,
    tokio::sync::broadcast::Sender<u8>,
    tokio::sync::watch::Sender<u8>,
) {
    let (standard, _) = std::sync::mpsc::channel();
    let (asynchronous, _) = tokio::sync::mpsc::channel(1);
    let (single, _) = tokio::sync::oneshot::channel();
    let (broadcast, _) = tokio::sync::broadcast::channel(1);
    let (watch, _) = tokio::sync::watch::channel(0);
    (standard, asynchronous, single, broadcast, watch)
}
pub fn errors(
    _: Option<std::sync::mpsc::RecvError>,
    _: Option<tokio::sync::mpsc::error::TryRecvError>,
    _: Option<tokio::sync::oneshot::error::RecvError>,
    _: Option<tokio::sync::broadcast::error::RecvError>,
    _: Option<tokio::sync::watch::error::RecvError>,
) {}
";

    support::accepted(source, &support::tokio())
}

#[test]
fn qualifies_channel_namespaces_without_constraining_other_tokio_types() -> Result<()> {
    let source = r"use std::sync::mpsc as std_mpsc;
use tokio::sync::{mpsc as tokio_mpsc, oneshot as tokio_oneshot, broadcast as fanout, watch as latest};
use tokio::runtime as executor;
pub fn standard() -> std_mpsc::Sender<u8> { std_mpsc::channel().0 }
pub fn asynchronous() -> tokio_mpsc::Sender<u8> { tokio_mpsc::channel(1).0 }
pub fn single() -> tokio_oneshot::Sender<u8> { tokio_oneshot::channel().0 }
pub fn broadcast() -> fanout::Sender<u8> { fanout::channel(1).0 }
pub fn watch() -> latest::Receiver<u8> { latest::channel(0).1 }
pub fn errors(_: fanout::error::TryRecvError, _: latest::error::RecvError) {}
pub fn handle() -> executor::Handle { executor::Handle::current() }
pub async fn task() -> tokio::task::JoinHandle<()> { tokio::task::spawn(async {}) }
";

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(
        fixed,
        source
            .replace("std_mpsc::", "std::sync::mpsc::")
            .replace("tokio_mpsc::", "tokio::sync::mpsc::")
            .replace("tokio_oneshot::", "tokio::sync::oneshot::")
            .replace("fanout::", "tokio::sync::broadcast::")
            .replace("latest::", "tokio::sync::watch::")
    );

    Ok(())
}

#[test]
fn qualifies_bare_channel_bindings_and_preserves_the_original_import() -> Result<()> {
    let source = r"use tokio::sync::mpsc::{channel, Sender};
pub fn create() -> Sender<u8> { channel(1).0 }
";

    let (_, fixed) = support::rewritten(source, &support::tokio())?;
    assert_eq!(
        fixed,
        source
            .replace("-> Sender", "-> tokio::sync::mpsc::Sender")
            .replace("{ channel(", "{ tokio::sync::mpsc::channel(")
    );

    Ok(())
}
