use anyhow::Result;
use pathkit::{
    SyncFsOps,
    path,
};

mod support;

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

    let directory = support::fixture(source)?;
    let output = support::run(&directory, true, "kiki_import_path_prefix")?;
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

    assert_eq!(support::source(&directory)?, expected);

    let output = support::run(&directory, false, "kiki_import_path_prefix")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_import_path_prefix").is_empty(),
        "{output:?}"
    );

    Ok(())
}

#[test]
fn grouped_paths_and_block_local_bindings_keep_their_meaning() -> Result<()> {
    let source = r"#![cfg_attr(test, allow(dead_code))]
mod a {
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

    let directory = support::fixture(source)?;
    let output = support::run(&directory, true, "kiki_import_path_prefix")?;
    assert!(output.status.success(), "{output:?}");
    let fixed = support::source(&directory)?;
    // Conflicting shared prefixes are split before independent prefix fixes.
    assert_eq!(
        fixed,
        source
            .replace(
                "use crate::a::{Value as Parent, b::Local as Own};",
                "use super::Value as Parent;\nuse self::Local as Own;"
            )
            .replace("crate::a::Value as Separate", "super::Value as Separate")
            .replace("crate::a::{Value as Uniform", "super::{Value as Uniform")
            .replace("crate::a::*", "super::*")
    );

    let output = support::run(&directory, false, "kiki_import_path_prefix")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_import_path_prefix").is_empty(),
        "{output:?}"
    );

    Ok(())
}

#[test]
fn physical_file_names_do_not_determine_module_ancestry() -> Result<()> {
    let directory = support::fixture("mod a { pub struct Value; #[path = \"../odd.rs\"] mod nested; }\n")?;
    // rustc resolves #[path] relative to the inline module's directory.
    (path!(directory.path()) / "src" / "a").create_dir_sync()?;
    let file = path!(directory.path()) / "src" / "odd.rs";
    file.write_sync("use crate::a::Value as Alias;\nuse Alias as Second;\n")?;
    let output = support::run(&directory, true, "kiki_import_path_prefix")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        file.read_to_string_sync()?,
        "use super::Value as Alias;\nuse self::Alias as Second;\n"
    );

    let output = support::run(&directory, false, "kiki_import_path_prefix")?;
    assert!(output.status.success(), "{output:?}");
    assert!(
        support::diagnostics(&output, "kiki_import_path_prefix").is_empty(),
        "{output:?}"
    );

    Ok(())
}
