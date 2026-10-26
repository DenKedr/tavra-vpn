#[test]
fn names_and_bounds_are_validated() {
    assert!(tavra_control::users::validate_contact(&"x".repeat(121), "", "", "").is_err());
    assert!(tavra_control::users::validate_contact("Denis", "", "", "").is_ok());
}
