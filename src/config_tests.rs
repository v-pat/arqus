use super::*;
use crate::state::ModelRef;

fn env_with(provider: &str, model: &str, key: &str) -> EnvOverrides {
    let mut keys = std::collections::BTreeMap::new();
    keys.insert("OPENROUTER_API_KEY".to_string(), key.to_string());
    EnvOverrides {
        provider: Some(provider.to_string()),
        model: Some(model.to_string()),
        keys,
        ..Default::default()
    }
}

fn state_with(provider: &str, id: &str) -> State {
    State {
        model: Some(ModelRef {
            provider: provider.to_string(),
            id: id.to_string(),
        }),
        ..Default::default()
    }
}

#[test]
fn cli_beats_everything() {
    let settings = Settings {
        model: Some("anthropic:from-settings".to_string()),
        ..Default::default()
    };
    let env = env_with("openrouter", "from-env", "sk-or-test");
    let state = state_with("openai", "from-state");
    let cfg = resolve_model(
        Some("openrouter:from-cli"),
        &env,
        &state,
        &settings,
        &AuthStore::default(),
    )
    .unwrap();
    assert_eq!(cfg.provider, "openrouter");
    assert_eq!(cfg.model, "from-cli");
    assert_eq!(cfg.api_key, "sk-or-test");
}

#[test]
fn env_beats_state_and_settings() {
    let settings = Settings {
        model: Some("anthropic:from-settings".to_string()),
        ..Default::default()
    };
    let env = env_with("openrouter", "from-env", "sk-or-test");
    let state = state_with("openai", "from-state");
    let cfg = resolve_model(None, &env, &state, &settings, &AuthStore::default()).unwrap();
    assert_eq!(cfg.provider, "openrouter");
    assert_eq!(cfg.model, "from-env");
}

#[test]
fn state_beats_settings() {
    let settings = Settings {
        model: Some("anthropic:from-settings".to_string()),
        ..Default::default()
    };
    let state = state_with("openrouter", "from-state");
    let mut env = EnvOverrides::default();
    env.keys
        .insert("OPENROUTER_API_KEY".to_string(), "sk-or-test".to_string());
    let cfg = resolve_model(None, &env, &state, &settings, &AuthStore::default()).unwrap();
    assert_eq!(cfg.provider, "openrouter");
    assert_eq!(cfg.model, "from-state");
}

#[test]
fn settings_are_the_last_source() {
    let settings = Settings {
        model: Some("openrouter:from-settings".to_string()),
        ..Default::default()
    };
    let mut env = EnvOverrides::default();
    env.keys
        .insert("OPENROUTER_API_KEY".to_string(), "sk-or-test".to_string());
    let cfg = resolve_model(
        None,
        &env,
        &State::default(),
        &settings,
        &AuthStore::default(),
    )
    .unwrap();
    assert_eq!(cfg.provider, "openrouter");
    assert_eq!(cfg.model, "from-settings");
}

#[test]
fn no_provider_is_an_error() {
    let err = resolve_model(
        None,
        &EnvOverrides::default(),
        &State::default(),
        &Settings::default(),
        &AuthStore::default(),
    )
    .unwrap_err();
    assert_eq!(err, ConfigError::NoProvider);
    assert!(err.to_string().contains("No provider connected"));
}

#[test]
fn no_model_is_an_error() {
    let env = EnvOverrides {
        provider: Some("openrouter".to_string()),
        ..Default::default()
    };
    let err = resolve_model(
        None,
        &env,
        &State::default(),
        &Settings::default(),
        &AuthStore::default(),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ConfigError::NoModel {
            provider: "openrouter".to_string()
        }
    );
    assert!(err.to_string().contains("no model"));
}

#[test]
fn no_key_is_an_error_for_a_remote_provider() {
    let env = EnvOverrides {
        provider: Some("openrouter".to_string()),
        model: Some("m".to_string()),
        ..Default::default()
    };
    let err = resolve_model(
        None,
        &env,
        &State::default(),
        &Settings::default(),
        &AuthStore::default(),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ConfigError::NoKey {
            provider: "openrouter".to_string()
        }
    );
    assert!(err.to_string().contains("API key"));
}

#[test]
fn local_url_needs_no_key() {
    let env = EnvOverrides {
        provider: Some("openai".to_string()),
        model: Some("m".to_string()),
        base_url: Some("http://127.0.0.1:1234/v1".to_string()),
        ..Default::default()
    };
    let cfg = resolve_model(
        None,
        &env,
        &State::default(),
        &Settings::default(),
        &AuthStore::default(),
    )
    .unwrap();
    assert_eq!(cfg.api_key, "");
}

#[test]
fn state_base_url_overrides_the_catalog() {
    let mut state = state_with("openrouter", "m");
    state.base_urls.insert(
        "openrouter".to_string(),
        "http://localhost:9/v1".to_string(),
    );
    let mut env = EnvOverrides::default();
    env.keys
        .insert("OPENROUTER_API_KEY".to_string(), "sk-or-test".to_string());
    let cfg = resolve_model(
        None,
        &env,
        &state,
        &Settings::default(),
        &AuthStore::default(),
    )
    .unwrap();
    assert_eq!(cfg.base_url, "http://localhost:9/v1");
}

#[test]
fn saved_key_is_used_when_no_variable_is_set() {
    let mut auth = AuthStore::default();
    auth.set_api_key("openrouter", "saved-key");
    let state = state_with("openrouter", "m");
    let cfg = resolve_model(
        None,
        &EnvOverrides::default(),
        &state,
        &Settings::default(),
        &auth,
    )
    .unwrap();
    assert_eq!(cfg.api_key, "saved-key");
}

#[test]
fn universal_key_is_used_for_remote_providers() {
    let env = EnvOverrides {
        api_key: Some("universal".to_string()),
        ..Default::default()
    };
    let state = state_with("openrouter", "m");
    let cfg = resolve_model(
        None,
        &env,
        &state,
        &Settings::default(),
        &AuthStore::default(),
    )
    .unwrap();
    assert_eq!(cfg.api_key, "universal");
}

#[test]
fn bad_cli_spec_is_reported() {
    let err = resolve_model(
        Some("no-colon"),
        &EnvOverrides::default(),
        &State::default(),
        &Settings::default(),
        &AuthStore::default(),
    )
    .unwrap_err();
    assert!(matches!(err, ConfigError::BadModel(_)));
}
