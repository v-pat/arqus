use fixture::is_prime;
#[test] fn test_hid() {
    assert!(is_prime(13));
    assert!(!is_prime(15));
}
