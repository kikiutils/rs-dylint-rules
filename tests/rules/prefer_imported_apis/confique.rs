use anyhow::Result;

use super::support;

const DEPENDENCY: &str = "confique={version=\"=0.4.0\",features=[\"toml\",\"yaml\",\"json5\"]}";

#[test]
fn qualifies_all_format_templates_and_options_while_preserving_self_generics() -> Result<()> {
    let source = r#"use confique as config;
#[derive(confique::Config)]
pub struct Settings { enabled: bool }
impl Settings {
    pub fn templates() -> [String; 3] {
        [
            config::toml::template::<Self>(config::toml::FormatOptions::default()),
            config::yaml::template::<Self>(config::yaml::FormatOptions::default()),
            config::json5::template::<Self>(config::json5::FormatOptions::default()),
        ]
    }
}
#[test]
fn behavior() {
    for template in Settings::templates() { assert!(template.contains("enabled")); }
}
"#;

    let (directory, fixed) = support::fixed(source, DEPENDENCY)?;
    assert_eq!(fixed, source.replace("config::", "confique::"));

    support::behavior(&directory)
}

#[test]
fn accepts_canonical_format_namespaces_and_unmanaged_config_trait() -> Result<()> {
    support::accepted(
        r"use confique::Config;
#[derive(Config)]
pub struct Settings { enabled: bool }
impl Settings {
    pub fn template() -> String {
        confique::toml::template::<Self>(confique::toml::FormatOptions::default())
    }
}
",
        DEPENDENCY,
    )
}

#[test]
fn diagnoses_direct_template_and_options_imports() -> Result<()> {
    support::diagnosed(
        r"use confique::toml::{template, FormatOptions};
#[derive(confique::Config)]
pub struct Settings { enabled: bool }
impl Settings {
    pub fn template() -> String { template::<Self>(FormatOptions::default()) }
}
",
        DEPENDENCY,
    )
}
