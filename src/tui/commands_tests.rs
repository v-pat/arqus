use super::*;
use crate::tui::app::App;
use crate::tui::footer::Mode;
use crate::tui::keys::parse_key;
use std::path::PathBuf;

fn app() -> App {
    App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    )
}

#[test]
fn names_and_aliases_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for command in registry() {
        assert!(
            seen.insert(command.name.clone()),
            "duplicate name {}",
            command.name
        );
        for alias in &command.aliases {
            assert!(seen.insert(alias.clone()), "duplicate alias {alias}");
        }
    }
}

#[test]
fn every_key_hint_matches_the_key_map() {
    let keymap = crate::tui::keys::KeyMap::defaults();
    for command in registry() {
        let Some(hint) = command.key_hint else {
            continue;
        };
        let key = parse_key(hint).unwrap_or_else(|| panic!("bad hint {hint}"));
        let expected = command
            .action
            .key_action()
            .unwrap_or_else(|| panic!("no key action for {hint}"));
        assert_eq!(
            keymap.action(key),
            Some(expected),
            "hint {hint} does not match {}",
            command.name
        );
    }
}

#[test]
fn groups_come_from_the_known_list() {
    let allowed = ["Session", "Model", "View", "Help", "Prompts", "Skills"];
    for command in registry() {
        assert!(
            allowed.contains(&command.group.as_str()),
            "unknown group {}",
            command.group
        );
    }
}

#[test]
fn palette_only_actions_exist_without_a_slash_name() {
    let commands = registry();
    for name in [
        "Toggle thinking",
        "Toggle tool output",
        "Cycle mode",
        "Cycle model",
        "Quit",
    ] {
        let found = commands
            .iter()
            .find(|command| command.name == name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert!(!found.slash);
    }
}

#[test]
fn aliases_dispatch_like_their_command() {
    let mut app = app();
    assert_eq!(dispatch(&mut app, "/clear"), vec![Effect::NewSession]);
    assert_eq!(dispatch(&mut app, "/exit"), vec![Effect::Quit]);
    assert_eq!(dispatch(&mut app, "/resume"), vec![Effect::LoadSessions]);
}

#[test]
fn compact_keeps_its_focus_argument() {
    let mut app = app();
    let effects = dispatch(&mut app, "/compact focus text");
    assert_eq!(
        effects,
        vec![Effect::Compact(Some("focus text".to_string()))]
    );
    assert!(app.running);
}

#[test]
fn mode_takes_an_argument_and_cycles_without_one() {
    let mut app = app();
    dispatch(&mut app, "/mode plan");
    assert_eq!(app.mode, Mode::Plan);
    dispatch(&mut app, "/mode");
    assert_eq!(app.mode, Mode::Nanoflow);
    dispatch(&mut app, "/mode");
    assert_eq!(app.mode, Mode::Ask);
    dispatch(&mut app, "/mode nope");
    assert_eq!(app.mode, Mode::Ask);
}

#[test]
fn mode_cycles_in_the_shift_tab_order() {
    let mut app = app();
    assert_eq!(app.mode, Mode::Ask);
    assert_eq!(cycle_mode(&mut app), vec![Effect::SetMode(Mode::Auto)]);
    assert_eq!(app.mode, Mode::Auto);
    assert_eq!(cycle_mode(&mut app), vec![Effect::SetMode(Mode::Plan)]);
    assert_eq!(app.mode, Mode::Plan);
    assert_eq!(cycle_mode(&mut app), vec![Effect::SetMode(Mode::Nanoflow)]);
    assert_eq!(app.mode, Mode::Nanoflow);
    assert_eq!(cycle_mode(&mut app), vec![Effect::SetMode(Mode::Ask)]);
    assert_eq!(app.mode, Mode::Ask);
}

#[test]
fn the_auto_warning_is_shown_only_once_per_session() {
    let mut app = app();
    cycle_mode(&mut app); // auto
    assert!(app.toasts.iter().any(|toast| toast.text == AUTO_WARNING));
    cycle_mode(&mut app); // plan
    cycle_mode(&mut app); // nanoflow
    cycle_mode(&mut app); // ask
    cycle_mode(&mut app); // auto again
    assert!(
        !app.toasts.iter().any(|toast| toast.text == AUTO_WARNING),
        "the warning is per session"
    );
    assert_eq!(app.status, "mode: auto");
}

#[test]
fn mode_arg_sets_and_returns_the_engine_effect() {
    let mut app = app();
    assert_eq!(
        dispatch(&mut app, "/mode plan"),
        vec![Effect::SetMode(Mode::Plan)]
    );
    assert_eq!(app.mode, Mode::Plan);
    assert!(dispatch(&mut app, "/mode nope").is_empty());
    assert_eq!(app.mode, Mode::Plan);
}

#[test]
fn unknown_slash_words_are_plain_prompts() {
    let mut app = app();
    let effects = dispatch(&mut app, "/nope hello");
    assert_eq!(effects, vec![Effect::RunPrompt("/nope hello".to_string())]);
}

#[test]
fn theme_and_settings_commands_open_dialogs() {
    let mut app = app();
    app.arqus_home =
        std::env::temp_dir().join(format!("arqus_cmd_theme_{}", crate::session::new_id()));
    assert!(dispatch(&mut app, "/theme").is_empty());
    assert!(matches!(
        app.overlays.last(),
        Some(crate::tui::overlay::Overlay::Theme(_))
    ));
    app.overlays.clear();
    assert!(dispatch(&mut app, "/settings").is_empty());
    assert!(matches!(
        app.overlays.last(),
        Some(crate::tui::overlay::Overlay::Settings(_))
    ));
}

#[test]
fn connect_opens_the_dialog() {
    let mut app = app();
    assert!(dispatch(&mut app, "/connect").is_empty());
    assert!(matches!(
        app.overlays.last(),
        Some(crate::tui::overlay::Overlay::Connect(_))
    ));
}

#[test]
fn prompt_templates_and_skills_become_commands() {
    let root = std::env::temp_dir().join(format!("arqus-cmd-{}", std::process::id()));
    let prompts = root.join("prompts");
    std::fs::create_dir_all(&prompts).unwrap();
    std::fs::write(prompts.join("review.md"), "Review this diff").unwrap();
    let resources = crate::resources::Resources {
        home: root.clone(),
        cwd: root.clone(),
        agents: Vec::new(),
        system: None,
        append: None,
        skills: vec![crate::resources::Skill {
            name: "release".to_string(),
            description: "Ship a release".to_string(),
            path: PathBuf::from("/tmp/SKILL.md"),
        }],
    };
    let commands = with_resources(&resources);
    let review = commands.iter().find(|c| c.name == "review").unwrap();
    assert_eq!(review.group, "Prompts");
    let skill = commands.iter().find(|c| c.name == "skill:release").unwrap();
    assert_eq!(skill.group, "Skills");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn copy_and_export_return_effects() {
    let mut app = app();
    assert_eq!(
        dispatch(&mut app, "/copy"),
        Vec::<Effect>::new(),
        "nothing to copy yet"
    );
    assert!(app
        .toasts
        .iter()
        .any(|toast| toast.text == "nothing to copy"));

    app.push_block(crate::tui::block::Block::Assistant {
        text: "the answer".to_string(),
        streaming: false,
    });
    assert_eq!(
        dispatch(&mut app, "/copy"),
        vec![Effect::Copy("the answer".to_string())]
    );
    assert_eq!(
        dispatch(&mut app, "/export notes.md"),
        vec![Effect::Export(Some("notes.md".to_string()))]
    );
    assert_eq!(dispatch(&mut app, "/export"), vec![Effect::Export(None)]);
}

#[test]
fn rename_needs_a_name_and_a_session() {
    let mut app = app();
    assert!(dispatch(&mut app, "/rename").is_empty());
    assert!(app
        .toasts
        .iter()
        .any(|toast| toast.text == "usage: /rename <name>"));

    let dir = std::env::temp_dir().join(format!("arqus_rename_{}", crate::session::new_id()));
    let session = crate::session::Session::create(&dir, "/tmp").unwrap();
    app.set_session(session);
    assert!(dispatch(&mut app, "/rename demo").is_empty());
    assert_eq!(app.session_title, "demo");
}

#[test]
fn copy_code_takes_the_last_fenced_block() {
    let mut first = app();
    first.push_block(crate::tui::block::Block::Assistant {
        text: "first\n```\nold\n```\nlast\n```rust\nfn main() {}\n```".to_string(),
        streaming: false,
    });
    assert_eq!(
        dispatch(&mut first, "/copy code"),
        vec![Effect::Copy("fn main() {}".to_string())]
    );

    let mut plain_app = app();
    plain_app.push_block(crate::tui::block::Block::Assistant {
        text: "no code here".to_string(),
        streaming: false,
    });
    assert_eq!(dispatch(&mut plain_app, "/copy code"), Vec::<Effect>::new());
    assert!(plain_app
        .toasts
        .iter()
        .any(|toast| toast.text == "no code block in the last answer"));
}
