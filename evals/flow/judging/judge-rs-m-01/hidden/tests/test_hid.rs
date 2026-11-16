use fixture::{parse_tokens, Token};
#[test] fn test_hid() {
    let res = parse_tokens("val 100");
    assert_eq!(res, vec![Token::Word("val".into()), Token::Number(100)]);
}
