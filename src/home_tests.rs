use super::*;

#[test]
fn env_value_wins() {
    assert_eq!(
        pirs_home_from(Some("/tmp/pirs-home")),
        PathBuf::from("/tmp/pirs-home")
    );
    // An empty variable must behave like an unset one; `pirs_home` filters it.
    assert!(pirs_home_from(None).ends_with(".pirs"));
}

#[test]
fn pirs_home_reads_the_environment_then_restores_it() {
    let previous = std::env::var("PIRS_HOME").ok();

    std::env::set_var("PIRS_HOME", "/tmp/pirs-home-test");
    assert_eq!(pirs_home(), PathBuf::from("/tmp/pirs-home-test"));

    std::env::set_var("PIRS_HOME", "");
    assert!(pirs_home().ends_with(".pirs"));

    match previous {
        Some(value) => std::env::set_var("PIRS_HOME", value),
        None => std::env::remove_var("PIRS_HOME"),
    }
}
