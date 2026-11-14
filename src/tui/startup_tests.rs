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

fn dummy_startup(default_mode: Option<&str>, plan: bool, auto: bool) -> Startup {
    let dir = std::env::temp_dir().join(format!("arqus_startup_{}", crate::session::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let session = crate::session::Session::create(&dir, &dir.display().to_string()).unwrap();
    let cfg = crate::agent::AgentConfig {
        cwd: dir.clone(),
        system_prompt: "test".to_string(),
        context_window: 128_000,
        compaction: crate::compaction::CompactionSettings::default(),
        limits: crate::cost::Limits::default(),
        approver: crate::safety::Approver::Interactive,
        allow_risky: false,
        plan,
        approvals: crate::safety::Approvals::default(),
        guard: crate::safety::Guard {
            cwd: dir.clone(),
            allow_paths: Vec::new(),
            arqus_home: dir.clone(),
            user_home: None,
        },
        mcp: None,
    };
    let runtime = crate::agent::Runtime {
        llm: crate::ai::Llm::Mock(std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::VecDeque::new(),
        ))),
        cfg,
        model_name: "test-model".to_string(),
    };
    let state = State {
        default_mode: default_mode.map(str::to_string),
        ..Default::default()
    };
    Startup {
        runtime,
        session,
        session_dir: dir.clone(),
        cwd: dir.display().to_string(),
        resources: crate::resources::Resources {
            home: dir.clone(),
            cwd: dir.clone(),
            agents: Vec::new(),
            system: None,
            append: None,
            skills: Vec::new(),
        },
        provider: "openrouter".to_string(),
        auto,
        connect_on_start: false,
        models_on_start: false,
        resume_on_start: false,
        auth: crate::auth::AuthStore::default(),
        state,
        env: crate::settings::EnvOverrides::default(),
        settings: crate::settings::Settings::default(),
        arqus_home: dir,
    }
}

#[test]
fn init_app_syncs_auto_mode_from_state_default_mode() {
    let startup = dummy_startup(Some("auto"), false, false);
    let (app, runtime, _) = init_app(startup);
    assert_eq!(app.mode, footer::Mode::Auto);
    assert_eq!(runtime.cfg.approver, crate::safety::Approver::Auto);
    assert!(!runtime.cfg.plan);
}

#[test]
fn init_app_syncs_plan_mode_when_cfg_plan_is_true() {
    let startup = dummy_startup(None, true, false);
    let (app, runtime, _) = init_app(startup);
    assert_eq!(app.mode, footer::Mode::Plan);
    assert!(runtime.cfg.plan);
    assert_eq!(runtime.cfg.approver, crate::safety::Approver::Interactive);
}

#[test]
fn init_app_defaults_to_ask_mode() {
    let startup = dummy_startup(None, false, false);
    let (app, runtime, _) = init_app(startup);
    assert_eq!(app.mode, footer::Mode::Ask);
    assert_eq!(runtime.cfg.approver, crate::safety::Approver::Interactive);
    assert!(!runtime.cfg.plan);
}
