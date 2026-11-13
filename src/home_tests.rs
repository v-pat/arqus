use super::*;

#[test]
fn env_value_wins() {
    assert_eq!(
        arqus_home_from(Some("/tmp/arqus-home")),
        PathBuf::from("/tmp/arqus-home")
    );
    // An empty variable must behave like an unset one; `arqus_home` filters it.
    assert!(arqus_home_from(None).ends_with(".arqus"));
}

#[test]
fn arqus_home_reads_the_environment_then_restores_it() {
    let previous = std::env::var("ARQUS_HOME").ok();

    std::env::set_var("ARQUS_HOME", "/tmp/arqus-home-test");
    assert_eq!(arqus_home(), PathBuf::from("/tmp/arqus-home-test"));

    std::env::set_var("ARQUS_HOME", "");
    assert!(arqus_home().ends_with(".arqus"));

    match previous {
        Some(value) => std::env::set_var("ARQUS_HOME", value),
        None => std::env::remove_var("ARQUS_HOME"),
    }
}
