// Each integration test binary uses a subset of these shared fixture helpers.
#![allow(dead_code)]

use std::process::{
    Command,
    Output,
};

use anyhow::Result;
use pathkit::{
    SyncFsOps,
    path,
};
use tempfile::TempDir;

pub(crate) const LINTS: &[&str] = &[
    "kiki_prefer_imported_apis",
    "kiki_import_path_prefix",
    "kiki_import_module_order",
    "kiki_grouped_derive_order",
    "kiki_statement_padding",
];

// Pinned so fixture builds stay reproducible.
pub(crate) const ANYHOW_VERSION: &str = "=1.0.104";

pub(crate) fn fixture(source: &str) -> Result<TempDir> {
    fixture_with(source, &[])
}

// Creates a fixture crate with `src/lib.rs` plus extra files relative to the crate root.
pub(crate) fn fixture_with(source: &str, files: &[(&str, &str)]) -> Result<TempDir> {
    let directory = tempfile::tempdir()?;
    write(
        &directory,
        "Cargo.toml",
        "[package]\nname=\"lint-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n",
    )?;

    write(&directory, "src/lib.rs", source)?;
    for (relative, content) in files {
        write(&directory, relative, content)?;
    }

    Ok(directory)
}

pub(crate) fn cargo(directory: &TempDir) -> Command {
    let mut command = Command::new("cargo");
    // Fixtures live outside the repository; inherit its toolchain explicitly.
    command
        .current_dir(directory.path())
        .env("RUSTUP_TOOLCHAIN", env!("RUSTUP_TOOLCHAIN"));

    command
}

pub(crate) fn run(directory: &TempDir, fix: bool, lint: &str) -> Result<Output> {
    run_lints(directory, fix, &[lint])
}

pub(crate) fn run_lints(directory: &TempDir, fix: bool, lints: &[&str]) -> Result<Output> {
    run_with_args(directory, fix, lints, &["--lib"])
}

pub(crate) fn run_with_args(directory: &TempDir, fix: bool, lints: &[&str], targets: &[&str]) -> Result<Output> {
    let mut command = cargo(directory);
    // Allow every kiki lint, then warn only the requested ones.
    let flags = std::iter::once("-A unused".to_owned())
        .chain(LINTS.iter().map(|lint| format!("-A {lint}")))
        .chain(lints.iter().map(|lint| format!("-W {lint}")))
        .collect::<Vec<_>>()
        .join(" ");

    command.env("DYLINT_RUSTFLAGS", flags).args([
        "dylint",
        "--path",
        env!("CARGO_MANIFEST_DIR"),
        "--fail-on-no-libraries",
    ]);

    if fix {
        command.arg("--fix");
    }

    command
        .arg("--")
        .args(targets)
        .args(["--message-format=json", "--quiet"]);

    if fix {
        command.args(["--allow-no-vcs", "--allow-dirty"]);
    }

    Ok(command.output()?)
}

pub(crate) fn messages(output: &Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-message")
        .collect()
}

pub(crate) fn diagnostics(output: &Output, lint: &str) -> Vec<serde_json::Value> {
    messages(output)
        .into_iter()
        .filter(|message| message["message"]["code"]["code"] == lint)
        .collect()
}

pub(crate) fn has_lint(output: &Output, lint: &str) -> bool {
    !diagnostics(output, lint).is_empty()
}

pub(crate) fn source(directory: &TempDir) -> Result<String> {
    read(directory, "src/lib.rs")
}

pub(crate) fn read(directory: &TempDir, relative: &str) -> Result<String> {
    Ok((path!(directory.path()) / relative).read_to_string_sync()?)
}

pub(crate) fn write(directory: &TempDir, relative: &str, content: &str) -> Result<()> {
    let file = path!(directory.path()) / relative;
    if let Some(parent) = file.parent() {
        parent.create_dir_all_sync()?;
    }

    file.write_sync(content)?;
    Ok(())
}

// Appends raw TOML sections to the fixture manifest.
pub(crate) fn manifest(directory: &TempDir, toml: &str) -> Result<()> {
    let manifest = read(directory, "Cargo.toml")?;
    write(directory, "Cargo.toml", &format!("{manifest}\n{toml}\n"))
}

pub(crate) fn dependency(directory: &TempDir, entries: &str) -> Result<()> {
    manifest(directory, &format!("[dependencies]\n{entries}"))
}

pub(crate) fn anyhow(directory: &TempDir) -> Result<()> {
    dependency(directory, &format!("anyhow=\"{ANYHOW_VERSION}\""))
}
