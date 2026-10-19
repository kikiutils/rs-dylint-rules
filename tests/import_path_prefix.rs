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

fn fixture(source: &str) -> Result<TempDir> {
    let directory = tempfile::tempdir()?;
    (path!(directory.path()) / "src").create_dir_sync()?;
    (path!(directory.path()) / "Cargo.toml")
        .write_sync("[package]\nname=\"prefix-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n")?;
    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
    Ok(directory)
}

fn run(directory: &TempDir, fix: bool) -> Result<Output> {
    let mut command = Command::new("cargo");
    command
        .env("DYLINT_RUSTFLAGS", "-A unused -A kiki_import_module_order")
        .current_dir(directory.path())
        .args([
            "+nightly-2026-10-01",
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

fn diagnostics(output: &Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["message"]["code"]["code"] == "kiki_import_path_prefix")
        .collect()
}

#[test]
fn fixes_resolved_routes_without_shortcutting_reexports() -> Result<()> {
    let source = r"use std::fmt;
mod other { pub struct Other; }
mod a {
    pub struct Ancestor;
    pub mod b { pub struct Sibling; }
    pub mod c {
        pub struct Parent;
        pub mod rr {
            pub mod child { pub struct Local; }
            pub use crate::a::c::rr::child::Local as Public;
            pub(crate) use crate::a::c::Parent as ParentAlias;
            use crate::a::b::Sibling;
            use super::super::super::other::Other;
            use crate::bridge::Local as ViaBridge;
            use child::Local as Bare;
            use super::rr::child::Local as Detour;
            use crate::a::c::rr::child::{Local as Grouped};
            fn nested() {
                use crate::a::c::rr::child::Local as Function;
                let _ = || { use crate::a::Ancestor; };
                let _ = async { use crate::a::c::Parent; };
            }
        }
    }
}
pub use crate::a::c::rr::child as bridge;
";
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let expected = source
        .replace("crate::a::c::rr::child", "self::child")
        .replace("crate::a::c::Parent as ParentAlias", "super::Parent as ParentAlias")
        .replace("crate::a::b::Sibling", "super::super::b::Sibling")
        .replace("super::super::super::other", "crate::other")
        .replace("use child::Local as Bare", "use self::child::Local as Bare")
        .replace("super::rr::child", "self::child")
        .replace("use crate::a::Ancestor", "use super::super::Ancestor")
        .replace("use crate::a::c::Parent", "use super::Parent")
        .replace(
            "pub use self::child as bridge",
            "pub use self::a::c::rr::child as bridge",
        );
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        expected
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    Ok(())
}

#[test]
fn grouped_paths_and_block_local_bindings_keep_their_meaning() -> Result<()> {
    let source = r"mod a {
    pub struct Value;
    pub struct Other;
    pub mod b {
        use crate::a::{Value as Parent, b::Local as Own};
        use crate::a::{Value as Uniform, Other as UniformOther};
        use crate::a::*;
        use {crate::a::Value as Separate, std::fmt};
        pub struct Local;
        fn f() {
            mod local { pub struct Value; }
            use local::Value;
        }
    }
}
";
    let directory = fixture(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    let fixed = (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?;
    // Conflicting shared prefixes need a use-tree rewrite and remain diagnostic-only.
    // Independent paths inside a rootless group can still be fixed.
    assert_eq!(
        fixed,
        source
            .replace("crate::a::Value as Separate", "super::Value as Separate")
            .replace("crate::a::{Value as Uniform", "super::{Value as Uniform")
            .replace("crate::a::*", "super::*")
    );
    assert_ne!(diagnostics(&run(&directory, false)?).len(), 0);
    Ok(())
}

#[test]
fn lint_levels_cfg_and_macro_expansions_are_respected() -> Result<()> {
    let source = r"mod a {
    pub struct Value;
    #[allow(kiki_import_path_prefix)]
    mod allowed { use crate::a::Value; }
    #[cfg(any())]
    mod inactive { use missing::Value; }
    macro_rules! generated { () => { mod generated { use crate::a::Value; } } }
    generated!();
    macro_rules! passthrough { ($($tokens:tt)*) => { $($tokens)* } }
    passthrough! { mod passed { use crate::a::Value; } }
    #[deny(kiki_import_path_prefix)]
    mod denied { use crate::a::Value; }
}
";
    let directory = fixture(source)?;
    let output = run(&directory, false)?;
    assert!(!output.status.success(), "{output:?}");
    let messages = diagnostics(&output);
    assert_eq!(messages.len(), 1, "{output:?}");
    assert_eq!(messages[0]["message"]["level"], "error");
    Ok(())
}

#[test]
fn physical_file_names_do_not_determine_module_ancestry() -> Result<()> {
    let directory = fixture("mod a { pub struct Value; #[path = \"../odd.rs\"] mod nested; }\n")?;
    // rustc resolves #[path] relative to the inline module's directory.
    (path!(directory.path()) / "src" / "a").create_dir_sync()?;
    let file = path!(directory.path()) / "src" / "odd.rs";
    file.write_sync("use crate::a::Value as Alias;\nuse Alias as Second;\n")?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        file.read_to_string_sync()?,
        "use super::Value as Alias;\nuse self::Alias as Second;\n"
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(diagnostics(&output).is_empty(), "{output:?}");
    Ok(())
}
