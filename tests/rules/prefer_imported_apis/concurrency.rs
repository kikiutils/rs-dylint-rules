use anyhow::Result;

use super::support;

#[test]
fn qualifies_task_and_thread_types_without_changing_unmanaged_types() -> Result<()> {
    let source = r"use std::thread as worker;
use tokio as runtime;
pub fn handles(
    _: worker::JoinHandle<()>,
    _: runtime::task::JoinHandle<()>,
    _: runtime::task::JoinError,
    _: worker::Thread,
    _: runtime::task::LocalSet,
) {
    let _ = worker::Builder::new();
    let _ = runtime::task::JoinSet::<()>::new();
}
";

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(
        fixed,
        format!(
            "use std::thread;\n{}",
            source
                .replace("worker::JoinHandle", "thread::JoinHandle")
                .replace("worker::Builder", "thread::Builder")
                .replace("runtime::task::Join", "tokio::task::Join")
        )
    );

    Ok(())
}

#[test]
fn distinguishes_standard_tokio_and_parking_lot_locks() -> Result<()> {
    let imports = "use std::sync::{Arc, Mutex as BlockingMutex, RwLock as BlockingRwLock};\nuse tokio::sync::{Mutex as AsyncMutex, RwLock as AsyncRwLock};\nuse parking_lot::{Mutex as ParkingMutex, RwLock as ParkingRwLock, MutexGuard};\n";
    let body = r"pub fn locks(
    _: Arc<u8>,
    _: BlockingMutex<u8>,
    _: BlockingRwLock<u8>,
    _: AsyncMutex<u8>,
    _: AsyncRwLock<u8>,
    _: ParkingMutex<u8>,
    _: ParkingRwLock<u8>,
    _: MutexGuard<'_, u8>,
) {
    let _ = BlockingMutex::new(1);
    let _ = BlockingRwLock::new(1);
    let _ = AsyncMutex::new(1);
    let _ = AsyncRwLock::new(1);
    let _ = ParkingMutex::new(1);
    let _ = ParkingRwLock::new(1);
}
";

    let dependencies = format!("{}\nparking_lot=\"=0.13.0\"", support::tokio());
    let (_, fixed) = support::fixed(&format!("{imports}{body}"), &dependencies)?;
    let mut expected = body.to_owned();
    for (original, canonical) in [
        ("BlockingMutex", "std::sync::Mutex"),
        ("BlockingRwLock", "std::sync::RwLock"),
        ("AsyncMutex", "tokio::sync::Mutex"),
        ("AsyncRwLock", "tokio::sync::RwLock"),
        ("ParkingMutex", "parking_lot::Mutex"),
        ("ParkingRwLock", "parking_lot::RwLock"),
    ] {
        expected = expected.replace(original, canonical);
    }

    assert_eq!(fixed, format!("{imports}{expected}"));
    Ok(())
}

#[test]
fn distinguishes_clock_types_and_still_imports_timer_functions() -> Result<()> {
    let imports = "use std::time::{Duration, Instant as SystemInstant};\nuse tokio::time::Instant as AsyncInstant;\n";
    let body = r"pub async fn clocks(_: SystemInstant, _: AsyncInstant) {
    let _ = SystemInstant::now();
    let _ = AsyncInstant::now();
    tokio::time::sleep(Duration::from_millis(1)).await;
}
";

    let (_, fixed) = support::fixed(&format!("{imports}{body}"), &support::tokio())?;
    assert_eq!(
        fixed,
        format!(
            "use tokio::time::sleep;\n{imports}{}",
            body.replace("SystemInstant", "std::time::Instant")
                .replace("AsyncInstant", "tokio::time::Instant")
                .replace("tokio::time::sleep", "sleep")
        )
    );

    Ok(())
}

#[test]
fn qualifies_standard_and_tokio_process_types_by_source() -> Result<()> {
    let source = r#"use std::process as standard;
use tokio::process as asynchronous;
pub fn processes(
    _: standard::Child,
    _: standard::ChildStdin,
    _: standard::ChildStdout,
    _: standard::ChildStderr,
    _: standard::Output,
    _: standard::ExitStatus,
    _: standard::ExitCode,
    _: asynchronous::Child,
    _: asynchronous::ChildStdin,
    _: asynchronous::ChildStdout,
    _: asynchronous::ChildStderr,
) {
    let _ = standard::Command::new("example");
    let _ = asynchronous::Command::new("example");
    let _ = standard::Stdio::null();
}
pub fn shared(_: std::process::Output, _: std::process::ExitStatus) {}
"#;

    let (_, fixed) = support::fixed(source, &support::tokio())?;
    assert_eq!(
        fixed,
        format!(
            "use std::process;\n{}",
            source
                .replace("standard::", "process::")
                .replace("asynchronous::", "tokio::process::")
                .replace("std::process::Output", "process::Output")
                .replace("std::process::ExitStatus", "process::ExitStatus")
        )
    );

    Ok(())
}
