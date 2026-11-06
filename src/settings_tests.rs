use super::*;

fn tmp() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pirs_settings_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

#[test]
fn missing_files_give_defaults() {
    let home = tmp();
    let project = tmp();
    assert_eq!(Settings::load_in(&home, &project), Settings::default());
}

#[test]
fn project_overrides_global_scalars() {
    let home = tmp();
    let project = tmp();
    write(
        &home.join(".pirs/settings.json"),
        r#"{"model":"anthropic:global","compaction":{"reserve_tokens":1000}}"#,
    );
    write(
        &project.join(".pirs/settings.json"),
        r#"{"base_url":"http://localhost:11434/v1","compaction":{"keep_recent_tokens":5000}}"#,
    );

    let found = Settings::load_in(&home, &project);
    assert_eq!(found.model.as_deref(), Some("anthropic:global"));
    assert_eq!(found.base_url.as_deref(), Some("http://localhost:11434/v1"));
    assert_eq!(found.compaction.reserve_tokens, Some(1000));
    assert_eq!(found.compaction.keep_recent_tokens, Some(5000));
}

#[test]
fn skill_folders_accumulate() {
    let home = tmp();
    let project = tmp();
    write(
        &home.join(".pirs/settings.json"),
        r#"{"skill_folders":["~/shared-skills"]}"#,
    );
    write(
        &project.join(".pirs/settings.json"),
        r#"{"skill_folders":["tools/skills"]}"#,
    );

    let found = Settings::load_in(&home, &project);
    assert_eq!(found.skill_folders, vec!["~/shared-skills", "tools/skills"]);
}

#[test]
fn broken_file_is_skipped_without_losing_the_other() {
    let home = tmp();
    let project = tmp();
    write(
        &home.join(".pirs/settings.json"),
        r#"{"model":"openai:gpt"}"#,
    );
    write(&project.join(".pirs/settings.json"), "{ not json");

    let found = Settings::load_in(&home, &project);
    assert_eq!(found.model.as_deref(), Some("openai:gpt"));
}

#[test]
fn compaction_overlay_keeps_untouched_keys() {
    let settings = Settings {
        compaction: CompactionOptions {
            enabled: Some(false),
            reserve_tokens: Some(4096),
            keep_recent_tokens: None,
        },
        ..Default::default()
    };
    let out = settings.compaction_settings(CompactionSettings::default());
    assert!(!out.enabled);
    assert_eq!(out.reserve_tokens, 4096);
    assert_eq!(out.keep_recent_tokens, 20_000);
}

#[test]
fn cli_flag_beats_env_beats_settings() {
    let settings = Settings {
        model: Some("anthropic:from-settings".to_string()),
        base_url: Some("http://settings".to_string()),
        context_window: Some(111),
        ..Default::default()
    };
    let env = EnvOverrides {
        model: Some("from-env".to_string()),
        api_key: Some("secret".to_string()),
        ..Default::default()
    };

    // CLI wins for provider and model.
    let cli = env.resolve(&settings, Some("openai:from-cli")).unwrap();
    assert_eq!(cli.provider, "openai");
    assert_eq!(cli.model, "from-cli");
    assert_eq!(cli.base_url, "http://settings");
    assert_eq!(cli.context_window, 111);
    assert_eq!(cli.api_key, "secret");

    // Without the flag the environment model wins, and the settings
    // provider is kept.
    let plain = env.resolve(&settings, None).unwrap();
    assert_eq!(plain.provider, "anthropic");
    assert_eq!(plain.model, "from-env");
}

#[test]
fn env_wins_over_settings_for_url_and_window() {
    let settings = Settings {
        base_url: Some("http://settings".to_string()),
        context_window: Some(111),
        ..Default::default()
    };
    let env = EnvOverrides {
        base_url: Some("http://env".to_string()),
        context_window: Some(222),
        ..Default::default()
    };
    let cfg = env.resolve(&settings, None).unwrap();
    assert_eq!(cfg.base_url, "http://env");
    assert_eq!(cfg.context_window, 222);
}

#[test]
fn empty_settings_use_provider_defaults() {
    let cfg = EnvOverrides::default()
        .resolve(&Settings::default(), None)
        .unwrap();
    assert_eq!(cfg.provider, "openai");
    assert_eq!(cfg.model, "");
    assert_eq!(cfg.base_url, default_base_url("openai"));
    assert_eq!(cfg.context_window, default_context_window("openai"));
    assert_eq!(cfg.api_key, "");
}

#[test]
fn bad_settings_model_is_an_error() {
    let settings = Settings {
        model: Some("no-colon".to_string()),
        ..Default::default()
    };
    assert!(EnvOverrides::default().resolve(&settings, None).is_err());
}
