use fixture::validate_username; #[test] fn test_hid() { assert!(!validate_username("")); assert!(!validate_username("a@b")); }
