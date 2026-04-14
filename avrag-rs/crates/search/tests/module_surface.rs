#[test]
fn search_lib_is_module_assembly_only() {
    let source = include_str!("../src/lib.rs");
    for forbidden_prefix in [
        "pub struct ",
        "struct ",
        "pub enum ",
        "enum ",
        "pub fn ",
        "fn ",
        "impl ",
    ] {
        assert!(
            !source
                .lines()
                .map(str::trim_start)
                .any(|line| line.starts_with(forbidden_prefix)),
            "lib.rs should not contain implementation item starting with `{forbidden_prefix}`",
        );
    }
}
