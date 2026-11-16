use fixture::SemVer;
#[test] fn test_hid() {
    let v = SemVer { major: 2, minor: 0, patch: 1 };
    assert_eq!(v.to_string(), "2.0.1");
}
