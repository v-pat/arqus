//! Base64 and OSC 52 tests. No clipboard program is started here.

use super::*;

#[test]
fn base64_matches_rfc_4648_vectors() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foob"), "Zm9vYg==");
    assert_eq!(base64(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn base64_handles_high_bytes() {
    assert_eq!(base64(&[0x00, 0x00, 0x00]), "AAAA");
    assert_eq!(base64(&[0xff, 0xff, 0xff]), "////");
    assert_eq!(base64("héllo".as_bytes()), "aMOpbGxv");
}

#[test]
fn osc52_wraps_the_payload() {
    assert_eq!(osc52("hi"), "\x1b]52;c;aGk=\x07");
    assert_eq!(osc52(""), "\x1b]52;c;\x07");
}
