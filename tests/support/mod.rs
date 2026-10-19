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

pub(crate) fn fixture(source: &str) -> Result<TempDir> {
    let directory = tempfile::tempdir()?;
    (path!(directory.path()) / "src").create_dir_sync()?;
    (path!(directory.path()) / "Cargo.toml")
        .write_sync("[package]\nname=\"lint-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n")?;

    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
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
    let mut command = cargo(directory);
    let flags = format!(
        "-A unused -A kiki_import_module_order -A kiki_import_path_prefix -A kiki_grouped_derive_order -A kiki_prefer_imported_apis -A kiki_statement_padding -W {lint}"
    );

    command.env("DYLINT_RUSTFLAGS", flags).args([
        "dylint",
        "--path",
        env!("CARGO_MANIFEST_DIR"),
        "--fail-on-no-libraries",
    ]);

    if fix {
        command.arg("--fix");
    }

    command.args(["--", "--lib", "--message-format=json", "--quiet"]);
    if fix {
        command.args(["--allow-no-vcs", "--allow-dirty"]);
    }

    Ok(command.output()?)
}

pub(crate) fn diagnostics(output: &Output, lint: &str) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["message"]["code"]["code"] == lint)
        .collect()
}
