use super::*;
use crate::models::ModelInfo;
use crate::tui::app::App;
use crate::tui::keys::{Key, KeyCodeX};
use crate::tui::msg::Msg;
use crate::tui::overlay::Overlay;
use crate::tui::testing::{assert_snapshot, render_to_string};
use crate::tui::update::update;
use std::path::PathBuf;

fn model(id: &str, tools: Option<bool>) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: id.to_string(),
        context: Some(128_000),
        prompt_per_mtok: Some(0.14),
        completion_per_mtok: Some(0.28),
        tools,
    }
}

fn models() -> Models {
    Models::loading("openrouter", &State::default(), "")
}

fn labels(models: &Models) -> Vec<String> {
    models
        .picker
        .items
        .iter()
        .map(|item| item.label.clone())
        .collect()
}

#[test]
fn typed_row_appears_only_without_an_exact_match() {
    let mut models = models();
    models.on_loaded(Ok(vec![
        model("alpha", Some(true)),
        model("beta", Some(true)),
    ]));

    models.picker.query = "alpha".to_string();
    models.rebuild();
    assert!(!labels(&models).iter().any(|label| label.contains("Use \"")));

    models.picker.query = "zzz".to_string();
    models.rebuild();
    assert!(labels(&models)
        .iter()
        .any(|label| label == "Use \"zzz\" as model id"));
}

#[test]
fn tab_filters_tool_capable_models() {
    let mut models = models();
    models.on_loaded(Ok(vec![
        model("a", Some(true)),
        model("b", Some(false)),
        model("c", None),
    ]));
    assert!(models.show_toggle);
    assert!(models.tools_only);
    assert_eq!(labels(&models), vec!["a"]);
    models.handle_key(Key::new(KeyCodeX::Tab));
    assert!(!models.tools_only);
    assert_eq!(labels(&models), vec!["a", "b", "c"]);
}

#[test]
fn no_tool_information_hides_the_toggle() {
    let mut models = models();
    models.on_loaded(Ok(vec![model("a", None), model("b", None)]));
    assert!(!models.show_toggle);
    models.handle_key(Key::new(KeyCodeX::Tab));
    assert_eq!(labels(&models), vec!["a", "b"]);
}

#[test]
fn selecting_a_model_emits_set_model() {
    let mut models = models();
    models.on_loaded(Ok(vec![model("alpha", Some(true))]));
    let outcome = models.handle_key(Key::new(KeyCodeX::Enter));
    assert_eq!(
        outcome,
        OverlayOutcome::CloseWith(vec![crate::tui::msg::Effect::SetModel {
            provider: "openrouter".to_string(),
            id: "alpha".to_string(),
        }])
    );
}

#[test]
fn models_loading_snapshot() {
    let mut app = App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        String::new(),
        128_000,
    );
    app.push_overlay(Overlay::Models(models()));
    let text = render_to_string(&mut app, 80, 24);
    assert_snapshot("models-loading-80x24", &text);
}

#[test]
fn models_loaded_snapshot() {
    let mut app = App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        String::new(),
        128_000,
    );
    let mut models = models();
    models.on_loaded(Ok(vec![
        model("deepseek/deepseek-v4-flash", Some(true)),
        model("openai/gpt-5", Some(true)),
        model("local/plain", Some(false)),
    ]));
    app.push_overlay(Overlay::Models(models));
    let text = render_to_string(&mut app, 80, 24);
    assert_snapshot("models-loaded-80x24", &text);
}

#[test]
fn models_error_with_typed_id_snapshot() {
    let mut app = App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        String::new(),
        128_000,
    );
    let mut models = models();
    models.on_loaded(Err("models: HTTP 500 provider trouble".to_string()));
    app.push_overlay(Overlay::Models(models));
    update(&mut app, Msg::Paste("my-model".to_string()));
    let text = render_to_string(&mut app, 80, 24);
    assert_snapshot("models-error-typed-80x24", &text);
    assert!(text.contains("Use \"my-model\" as model id"));
}
