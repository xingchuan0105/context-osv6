use avrag_share::AccessLevel;

#[test]
fn access_level_from_role_maps_editor_aliases_to_write() {
    assert_eq!(AccessLevel::from_role("editor"), AccessLevel::Write);
    assert_eq!(AccessLevel::from_role("write"), AccessLevel::Write);
    assert_eq!(AccessLevel::from_role("full"), AccessLevel::Write);
}

#[test]
fn access_level_share_management_requires_write_or_admin() {
    assert!(!AccessLevel::Read.allows_share_management());
    assert!(!AccessLevel::None.allows_share_management());
    assert!(AccessLevel::Write.allows_share_management());
    assert!(AccessLevel::Admin.allows_share_management());
}

#[test]
fn access_level_invite_role_contract() {
    assert_eq!(AccessLevel::Admin.as_invite_role(), "owner");
    assert_eq!(AccessLevel::Write.as_invite_role(), "editor");
    assert_eq!(AccessLevel::Read.as_invite_role(), "viewer");
}
