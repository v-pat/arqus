use super::*;
use crate::state::State;

fn ctx() -> overlay::connect::ConnectCtx {
    overlay::connect::ConnectCtx {
        current_provider: None,
        current_model: None,
        auth: crate::auth::AuthStore::default(),
        env: crate::settings::EnvOverrides::default(),
        base_urls: std::collections::BTreeMap::new(),
    }
}

#[test]
fn fresh_state_opens_connect_by_itself() {
    let overlays = startup_overlays(true, false, "", &State::default(), "", ctx());
    assert_eq!(overlays.len(), 1);
    match &overlays[0] {
        Overlay::Connect(connect) => {
            assert!(connect
                .heading
                .as_deref()
                .unwrap_or("")
                .contains("Welcome to arqus"));
        }
        _ => panic!("expected Connect"),
    }
}

#[test]
fn a_connected_provider_without_a_model_opens_models() {
    let overlays = startup_overlays(false, true, "openrouter", &State::default(), "", ctx());
    assert_eq!(overlays.len(), 1);
    assert!(matches!(overlays[0], Overlay::Models(_)));
}

#[test]
fn a_ready_config_opens_no_dialog() {
    let overlays = startup_overlays(false, false, "openrouter", &State::default(), "m", ctx());
    assert!(overlays.is_empty());
}
