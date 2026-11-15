use std::fs;

use proptest::prelude::*;
use tempfile::tempdir;

use super::*;

#[test]
fn test_dummy_property_runner() {
    // Included to anchor test module
}

proptest! {
    #[test]
    fn test_prop_search_edit_unique_substring(
        prefix in "[a-z]{5,10}",
        unique in "[0-9]{8}",
        suffix in "[a-z]{5,10}",
        replacement in "[A-Z]{3,6}",
    ) {
        let content = format!("{prefix}{unique}{suffix}");
        let dir = tempdir().unwrap();
        let p = dir.path().join("file.txt");
        fs::write(&p, &content).unwrap();
        let edits = vec![EditBlock::Search {
            path: "file.txt".into(),
            search: unique,
            replace: replacement.clone(),
        }];
        apply_edits(&edits, dir.path(), &["file.txt".into()]).unwrap();
        let expected = format!("{prefix}{replacement}{suffix}");
        assert_eq!(fs::read_to_string(p).unwrap(), expected);
    }

    #[test]
    fn test_prop_edit_parser_random_strings_never_panic(s in "\\PC*") {
        let _ = parse_edits(&s);
    }
}
