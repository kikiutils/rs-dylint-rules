use super::support;

#[test]
fn imports_time_functions_even_when_the_future_is_not_awaited_at_the_call_site() -> anyhow::Result<()> {
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
        source.replace("tokio::time::", "").replace(
            "    let delay =",
            "    use tokio::time::sleep;\n    use tokio::time::timeout;\n    use tokio::time::interval;\n    let delay =",
        )
    );
    Ok(())
}

#[test]
fn qualifies_task_functions_through_the_fixed_module_alias() -> anyhow::Result<()> {
    let source = r"pub async fn tasks() {
    let handle = tokio::spawn(async { 1 });
    let blocking = tokio::task::spawn_blocking(|| 2);
    tokio::task::yield_now().await;
    let _ = (handle.await, blocking.await);
}
";

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(
        fixed,
        source
            .replace("tokio::spawn", "tokio_task::spawn")
            .replace("tokio::task::", "tokio_task::")
            .replace(
                "    let handle =",
                "    use tokio::task as tokio_task;\n    let handle ="
            )
    );
    Ok(())
}

#[test]
fn qualifies_channel_functions_types_and_errors_with_their_fixed_aliases() -> anyhow::Result<()> {
    let source = r"pub fn channels() -> (
    std::sync::mpsc::Sender<u8>,
    tokio::sync::mpsc::Sender<u8>,
    tokio::sync::oneshot::Sender<u8>,
) {
    let (standard, _) = std::sync::mpsc::channel();
    let (asynchronous, _) = tokio::sync::mpsc::channel(1);
    let (single, _) = tokio::sync::oneshot::channel();
    (standard, asynchronous, single)
}
pub fn errors(
    _: Option<std::sync::mpsc::RecvError>,
    _: Option<tokio::sync::mpsc::error::TryRecvError>,
    _: Option<tokio::sync::oneshot::error::RecvError>,
) {}
";

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(
        fixed,
        format!(
            "use std::sync::mpsc as std_mpsc;\nuse tokio::sync::mpsc as tokio_mpsc;\nuse tokio::sync::oneshot as tokio_oneshot;\n{}",
            source
                .replace("std::sync::mpsc::", "std_mpsc::")
                .replace("tokio::sync::mpsc::", "tokio_mpsc::")
                .replace("tokio::sync::oneshot::", "tokio_oneshot::")
        )
    );
    Ok(())
}

#[test]
fn accepts_fixed_aliases_without_constraining_other_tokio_types() -> anyhow::Result<()> {
    support::accepted(
        r#"use std::sync::mpsc as std_mpsc;
use tokio::{sync::{mpsc as tokio_mpsc, oneshot as tokio_oneshot}, task as tokio_task};
use tokio::process as command_process;
pub fn standard() -> std_mpsc::Sender<u8> { std_mpsc::channel().0 }
pub fn asynchronous() -> tokio_mpsc::Sender<u8> { tokio_mpsc::channel(1).0 }
pub fn single() -> tokio_oneshot::Sender<u8> { tokio_oneshot::channel().0 }
pub fn command() -> command_process::Command { command_process::Command::new("example") }
pub async fn task() -> tokio::task::JoinHandle<()> { tokio_task::spawn(async {}) }
"#,
        &support::tokio(),
    )
}

#[test]
fn qualifies_bare_channel_bindings_without_changing_the_original_import() -> anyhow::Result<()> {
    let source = r"use tokio::sync::mpsc::{channel, Sender};
pub fn create() -> Sender<u8> { channel(1).0 }
";

    let (_, fixed) = support::rewritten(source, &support::tokio())?;
    assert_eq!(
        fixed,
        format!(
            "use tokio::sync::mpsc as tokio_mpsc;\n{}",
            source
                .replace("-> Sender", "-> tokio_mpsc::Sender")
                .replace("{ channel(", "{ tokio_mpsc::channel(")
        )
    );

    Ok(())
}
