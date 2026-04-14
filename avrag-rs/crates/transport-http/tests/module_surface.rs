#[test]
fn transport_http_lib_only_contains_module_assembly() {
    let lib_rs = include_str!("../src/lib.rs");
    assert!(!lib_rs.contains("struct RegisterRequest"));
    assert!(!lib_rs.contains("struct AuthEnvelope"));
    assert!(!lib_rs.contains("struct UpdateProfileRequest"));
    assert!(!lib_rs.contains("struct UserPreferencesPayload"));
}
