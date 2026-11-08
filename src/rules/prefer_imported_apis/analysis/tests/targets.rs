use super::*;

#[test]
fn cfg_boundaries_select_the_outermost_block_inside_the_nearest_boundary() {
    for (source, expected) in [
        (
            "#[cfg(all())] fn current() -> u32 { std::process::id() }",
            Some("{ std::process::id() }"),
        ),
        (
            "fn current() -> u32 { #[cfg(all())] let pid = { std::process::id() }; pid }",
            Some("{ std::process::id() }"),
        ),
        (
            "struct Process; #[cfg(all())] impl Process { fn current() -> u32 { std::process::id() } }",
            Some("{ std::process::id() }"),
        ),
        (
            "fn current() { #[cfg(all())] { let _ = std::process::id(); } }",
            Some("{ let _ = std::process::id(); }"),
        ),
        (
            "#[cfg(all())] trait Process { fn current() -> u32 { std::process::id() } }",
            Some("{ std::process::id() }"),
        ),
        (
            "#[cfg(all())] fn current() -> u32 { #[cfg(all())] let pid = { std::process::id() }; pid }",
            Some("{ std::process::id() }"),
        ),
        (
            "fn current() -> u32 { #[cfg(all())] let pid = std::process::id(); pid }",
            None,
        ),
        (
            "#[cfg_attr(all(), allow(dead_code))]\npub fn current() -> u32 { std::process::id() }\n",
            Some("#[cfg_attr(all(), allow(dead_code))]\npub fn current() -> u32 { std::process::id() }\n"),
        ),
    ] {
        let root = parse(source);
        let node = root
            .covering_element(range(source, "std::process::id"))
            .ancestors()
            .next()
            .unwrap_or_else(|| panic!("missing access: {source}"));

        let target = super::super::targets::target(&node).map(|node| node.text().to_string());
        assert_eq!(target.as_deref(), expected, "{source}");
    }
}
