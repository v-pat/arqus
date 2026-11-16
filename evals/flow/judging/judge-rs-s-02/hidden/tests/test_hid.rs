use fixture::binary_search;
#[test] fn test_hid() {
    assert_eq!(binary_search(&[2, 4, 6, 8], 8), Some(3));
    assert_eq!(binary_search(&[2, 4, 6, 8], 1), None);
}
