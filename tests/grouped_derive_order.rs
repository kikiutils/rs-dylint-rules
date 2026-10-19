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
        .write_sync("[package]\nname=\"derive-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n")?;
    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
    Ok(directory)
}

fn run(directory: &TempDir, fix: bool) -> Result<Output> {
    let mut command = Command::new("cargo");
    command
        .current_dir(directory.path())
        .env("DYLINT_RUSTFLAGS", "-A unused")
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
    command.args(["--", "--lib", "--quiet", "--message-format=json"]);
    if fix {
        command.args(["--allow-no-vcs", "--allow-dirty"]);
    }
    Ok(command.output()?)
}

fn has_lint(output: &Output) -> bool {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|message| message["message"]["code"]["code"] == "kiki_grouped_derive_order")
}

#[test]
fn fixes_builtin_lists_independently_and_resolves_aliases() -> Result<()> {
    let source = "use core::fmt::Debug as FormatDebug;\n#[derive(Copy, Clone)]\n#[derive(Hash, Eq, PartialEq, FormatDebug)]\npub struct Value(u32);\nfn nested() {\n    #[derive(Eq, PartialEq, Debug)]\n    struct Nested;\n}\n";
    let directory = fixture(source)?;
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(has_lint(&output), "{output:?}");
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        "use core::fmt::Debug as FormatDebug;\n#[derive(Clone, Copy)]\n#[derive(FormatDebug, PartialEq, Eq, Hash)]\npub struct Value(u32);\nfn nested() {\n    #[derive(Debug, PartialEq, Eq)]\n    struct Nested;\n}\n"
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!has_lint(&output), "{output:?}");
    Ok(())
}

#[test]
fn honors_lint_levels_and_inactive_items() -> Result<()> {
    let source = "#![deny(kiki_grouped_derive_order)]\n#[derive(Eq, PartialEq)] pub struct Value;\n";
    let directory = fixture(source)?;
    let output = run(&directory, false)?;
    assert!(!output.status.success());
    assert!(has_lint(&output), "{output:?}");
    let source = "#![allow(kiki_grouped_derive_order)]\n#[derive(Eq, PartialEq)] pub struct Value;\n#[cfg(any())] #[derive(Missing, Eq, PartialEq)] struct Inactive;\n";
    (path!(directory.path()) / "src" / "lib.rs").write_sync(source)?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!has_lint(&output), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source
    );
    Ok(())
}

#[test]
fn custom_macros_named_like_supported_derives_do_not_enable_sorting_their_lists() -> Result<()> {
    let source = "#[derive(Eq, custom::Clone, PartialEq)]\npub struct Protected;\n#[derive(Eq, custom::Serialize, PartialEq)]\npub struct ProtectedSerde;\n#[derive(Eq, custom::IsEmpty, PartialEq)]\npub struct ProtectedEmpty;\n#[derive(Eq, custom::Serialize_repr, PartialEq)]\npub struct ProtectedRepr;\n#[derive(Eq, custom::EnumCount, PartialEq)]\npub struct ProtectedStrum;\n#[derive(Hash, Eq, PartialEq)]\npub struct Plain;\n";
    let directory = fixture(source)?;
    let root = path!(directory.path());
    (&root / "custom" / "src").create_dir_all_sync()?;
    (&root / "custom" / "Cargo.toml")
        .write_sync("[package]\nname=\"custom\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\nproc-macro=true\n")?;
    (&root / "custom" / "src" / "lib.rs").write_sync("extern crate proc_macro;\n#[proc_macro_derive(Clone)] pub fn clone(_: proc_macro::TokenStream) -> proc_macro::TokenStream { proc_macro::TokenStream::new() }\n#[proc_macro_derive(Serialize)] pub fn serialize(_: proc_macro::TokenStream) -> proc_macro::TokenStream { proc_macro::TokenStream::new() }\n#[proc_macro_derive(IsEmpty)] pub fn empty(_: proc_macro::TokenStream) -> proc_macro::TokenStream { proc_macro::TokenStream::new() }\n#[proc_macro_derive(Serialize_repr)] pub fn repr(_: proc_macro::TokenStream) -> proc_macro::TokenStream { proc_macro::TokenStream::new() }\n#[proc_macro_derive(EnumCount)] pub fn count(_: proc_macro::TokenStream) -> proc_macro::TokenStream { proc_macro::TokenStream::new() }\n")?;
    let manifest = &root / "Cargo.toml";
    let text = manifest.read_to_string_sync()?;
    manifest.write_sync(format!("{text}\n[dependencies]\ncustom={{path=\"custom\"}}\n"))?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        (&root / "src" / "lib.rs").read_to_string_sync()?,
        "#[derive(Eq, custom::Clone, PartialEq)]\npub struct Protected;\n#[derive(Eq, custom::Serialize, PartialEq)]\npub struct ProtectedSerde;\n#[derive(Eq, custom::IsEmpty, PartialEq)]\npub struct ProtectedEmpty;\n#[derive(Eq, custom::Serialize_repr, PartialEq)]\npub struct ProtectedRepr;\n#[derive(Eq, custom::EnumCount, PartialEq)]\npub struct ProtectedStrum;\n#[derive(PartialEq, Eq, Hash)]\npub struct Plain;\n"
    );
    Ok(())
}

#[test]
fn all_builtin_derives_preserve_behavior_and_raw_source_offsets() -> Result<()> {
    let source = "const LABEL: &str = \"型別\";\r\n#[derive(Hash, Ord, Eq, Copy, PartialOrd, Default, Clone, Debug, PartialEq)]\r\npub struct Value(u32);\r\n#[test] fn behavior() { let value = Value::default(); assert_eq!(value, value.clone()); assert!(value < Value(1)); assert_eq!(format!(\"{value:?}\"), \"Value(0)\"); use std::hash::{Hash, Hasher}; let mut hasher = std::collections::hash_map::DefaultHasher::new(); value.hash(&mut hasher); assert_ne!(hasher.finish(), 0); }\r\n";
    let directory = fixture(source)?;
    for fix in [false, true] {
        let output = run(&directory, fix)?;
        assert!(output.status.success(), "{output:?}");
        let behavior = Command::new("cargo")
            .current_dir(directory.path())
            .args(["+nightly-2026-10-01", "test", "--lib", "--quiet"])
            .output()?;
        assert!(behavior.status.success(), "{behavior:?}");
    }
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source.replace(
            "Hash, Ord, Eq, Copy, PartialOrd, Default, Clone, Debug, PartialEq",
            "Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash"
        )
    );
    Ok(())
}

#[test]
fn shared_source_module_instances_keep_their_own_lint_levels() -> Result<()> {
    let directory = fixture(
        "#[allow(kiki_grouped_derive_order)]\n#[path=\"shared.rs\"] mod permitted;\n#[deny(kiki_grouped_derive_order)]\n#[path=\"shared.rs\"] mod forbidden;\n",
    )?;
    (path!(directory.path()) / "src" / "shared.rs").write_sync("#[derive(Eq, PartialEq)]\npub struct Value;\n")?;
    let output = run(&directory, false)?;
    assert!(!output.status.success());
    assert!(has_lint(&output), "{output:?}");
    Ok(())
}

#[test]
fn serde_aliases_and_qualified_paths_sort_and_preserve_roundtrip_behavior() -> Result<()> {
    let source = r##"use serde as renamed;
use renamed::{Deserialize as Decode, Serialize as Encode};
#[derive(Decode, Eq, Encode, Debug, PartialEq)]
pub struct Value { pub number: u32 }
#[derive(renamed::Deserialize, renamed::Serialize)]
pub enum State { Ready, Value(u32) }
#[test] fn roundtrip() {
    let value = Value { number: 7 };
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"number":7}"#);
    assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), value);
    let state = serde_json::to_string(&State::Value(9)).unwrap();
    assert_eq!(serde_json::to_string(&serde_json::from_str::<State>(&state).unwrap()).unwrap(), state);
}
"##;
    let directory = fixture(source)?;
    let manifest = path!(directory.path()) / "Cargo.toml";
    let text = manifest.read_to_string_sync()?;
    manifest.write_sync(format!(
        "{text}\n[dependencies]\nserde={{version=\"=1.0.229\", features=[\"derive\"]}}\nserde_json=\"=1.0.154\"\n"
    ))?;
    let file = path!(directory.path()) / "src" / "lib.rs";
    for fix in [false, true] {
        let output = run(&directory, fix)?;
        assert!(output.status.success(), "{output:?}");
        if !fix {
            assert!(has_lint(&output), "{output:?}");
        }
        let behavior = Command::new("cargo")
            .current_dir(directory.path())
            .args(["+nightly-2026-10-01", "test", "--lib", "--quiet"])
            .output()?;
        assert!(behavior.status.success(), "{behavior:?}");
    }
    assert_eq!(
        file.read_to_string_sync()?,
        source
            .replace(
                "Decode, Eq, Encode, Debug, PartialEq",
                "Debug, PartialEq, Eq, Encode, Decode"
            )
            .replace(
                "renamed::Deserialize, renamed::Serialize",
                "renamed::Serialize, renamed::Deserialize"
            )
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!has_lint(&output), "{output:?}");
    Ok(())
}

#[test]
fn renamed_serde_dependency_with_helper_attributes_is_fixed() -> Result<()> {
    let source = r#"#![allow(dead_code)]
#[cfg(all())]
mod nested {
#[derive(renamed::Deserialize, renamed::Serialize)]
#[serde(crate = "renamed", rename_all = "camelCase")]
pub struct Value { pub some_field: u32 }
}
"#;
    let directory = fixture(source)?;
    let manifest = path!(directory.path()) / "Cargo.toml";
    let text = manifest.read_to_string_sync()?;
    manifest.write_sync(format!(
        "{text}\n[dependencies]\nrenamed={{package=\"serde\", version=\"=1.0.229\", features=[\"derive\"]}}\n"
    ))?;
    let output = run(&directory, true)?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source.replace(
            "renamed::Deserialize, renamed::Serialize",
            "renamed::Serialize, renamed::Deserialize"
        )
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!has_lint(&output), "{output:?}");
    Ok(())
}

#[test]
fn supported_third_party_derives_preserve_behavior() -> Result<()> {
    let source = r#"use is_empty::IsEmpty as Empty;
use serde_repr::{Deserialize_repr as Decode, Serialize_repr as Encode};
use strum::{EnumCount as Count, VariantNames as Names};
#[derive(Empty, Default, Debug)]
pub struct Record {
    value: Option<u8>,
    #[is_empty(if = "Vec::is_empty")]
    extra: Vec<u8>,
}
#[derive(Decode, Encode, Debug, PartialEq)]
#[repr(u8)]
pub enum Number { One = 1, Two = 2 }
#[derive(strum::VariantArray, Names, Count, strum::EnumIs, Debug, PartialEq)]
pub enum State { Ready, Done }
#[test] fn behavior() {
    use is_empty::IsEmpty;
    assert!(Record::default().is_empty());
    assert!(!Record { value: Some(1), extra: vec![] }.is_empty());
    assert_eq!(serde_json::to_string(&Number::Two).unwrap(), "2");
    assert_eq!(serde_json::from_str::<Number>("1").unwrap(), Number::One);
    assert_eq!(<State as strum::EnumCount>::COUNT, 2);
    assert_eq!(<State as strum::VariantNames>::VARIANTS, &["Ready", "Done"]);
    assert_eq!(<State as strum::VariantArray>::VARIANTS, &[State::Ready, State::Done]);
    assert!(State::Ready.is_ready());
    assert!(!State::Done.is_ready());
}
"#;
    let directory = fixture(source)?;
    let manifest = path!(directory.path()) / "Cargo.toml";
    let text = manifest.read_to_string_sync()?;
    manifest.write_sync(format!("{text}\n[dependencies]\nis_empty=\"=0.3.0\"\nserde=\"=1.0.229\"\nserde_repr=\"=0.1.21\"\nserde_json=\"=1.0.154\"\nstrum={{version=\"=0.28.0\",default-features=false,features=[\"derive\"]}}\n"))?;
    for fix in [false, true] {
        let output = run(&directory, fix)?;
        assert!(output.status.success(), "{output:?}");
        if !fix {
            assert!(has_lint(&output), "{output:?}");
        }
        let behavior = Command::new("cargo")
            .current_dir(directory.path())
            .args(["+nightly-2026-10-01", "test", "--lib", "--quiet"])
            .output()?;
        assert!(behavior.status.success(), "{behavior:?}");
    }
    assert_eq!(
        (path!(directory.path()) / "src" / "lib.rs").read_to_string_sync()?,
        source
            .replace("Empty, Default, Debug", "Debug, Default, Empty")
            .replace("Decode, Encode, Debug, PartialEq", "Debug, PartialEq, Encode, Decode")
            .replace(
                "strum::VariantArray, Names, Count, strum::EnumIs, Debug, PartialEq",
                "Debug, PartialEq, strum::EnumIs, Count, Names, strum::VariantArray"
            )
    );
    let output = run(&directory, false)?;
    assert!(output.status.success(), "{output:?}");
    assert!(!has_lint(&output), "{output:?}");
    Ok(())
}
