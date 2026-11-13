//! The providers shown in the Connect dialog, and how a key is found for each.
//! URLs below were written from memory of each provider's docs: open each key_url once and fix any that moved.

use crate::auth::AuthStore;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiKind {
    OpenAiCompat,
    Anthropic,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: ApiKind,
    pub base_url: &'static str, // "" = the user types it (custom)
    pub env_vars: &'static [&'static str],
    pub key_url: &'static str,
    pub validate_path: &'static str, // GET this path (relative to the base URL) with the key to check it works
    pub needs_key: bool,
    pub recommended: bool,
    pub blurb: &'static str,
}

pub const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "openrouter",
        name: "OpenRouter",
        kind: ApiKind::OpenAiCompat,
        base_url: "https://openrouter.ai/api/v1",
        env_vars: &["OPENROUTER_API_KEY"],
        key_url: "https://openrouter.ai/settings/keys",
        validate_path: "/key",
        needs_key: true,
        recommended: true,
        blurb: "One key, hundreds of models, per-key spending limits",
    },
    ProviderInfo {
        id: "anthropic",
        name: "Anthropic",
        kind: ApiKind::Anthropic,
        base_url: "https://api.anthropic.com/v1",
        env_vars: &["ANTHROPIC_API_KEY"],
        key_url: "https://console.anthropic.com/settings/keys",
        validate_path: "/models",
        needs_key: true,
        recommended: true,
        blurb: "Claude models",
    },
    ProviderInfo {
        id: "openai",
        name: "OpenAI",
        kind: ApiKind::OpenAiCompat,
        base_url: "https://api.openai.com/v1",
        env_vars: &["OPENAI_API_KEY"],
        key_url: "https://platform.openai.com/api-keys",
        validate_path: "/models",
        needs_key: true,
        recommended: false,
        blurb: "GPT models",
    },
    ProviderInfo {
        id: "deepseek",
        name: "DeepSeek",
        kind: ApiKind::OpenAiCompat,
        base_url: "https://api.deepseek.com",
        env_vars: &["DEEPSEEK_API_KEY"],
        key_url: "https://platform.deepseek.com/api_keys",
        validate_path: "/models",
        needs_key: true,
        recommended: false,
        blurb: "DeepSeek models directly",
    },
    ProviderInfo {
        id: "groq",
        name: "Groq",
        kind: ApiKind::OpenAiCompat,
        base_url: "https://api.groq.com/openai/v1",
        env_vars: &["GROQ_API_KEY"],
        key_url: "https://console.groq.com/keys",
        validate_path: "/models",
        needs_key: true,
        recommended: false,
        blurb: "Very fast inference",
    },
    ProviderInfo {
        id: "ollama",
        name: "Ollama (local)",
        kind: ApiKind::OpenAiCompat,
        base_url: "http://localhost:11434/v1",
        env_vars: &[],
        key_url: "",
        validate_path: "/models",
        needs_key: false,
        recommended: false,
        blurb: "Models running on this machine, no key needed",
    },
    ProviderInfo {
        id: "lmstudio",
        name: "LM Studio (local)",
        kind: ApiKind::OpenAiCompat,
        base_url: "http://localhost:1234/v1",
        env_vars: &[],
        key_url: "",
        validate_path: "/models",
        needs_key: false,
        recommended: false,
        blurb: "Local server started from LM Studio",
    },
    ProviderInfo {
        id: "custom",
        name: "Custom OpenAI-compatible endpoint...",
        kind: ApiKind::OpenAiCompat,
        base_url: "",
        env_vars: &[],
        key_url: "",
        validate_path: "/models",
        needs_key: false, // optional for custom endpoints
        recommended: false,
        blurb: "Any server that speaks /chat/completions",
    },
];

/// Universal fallback variable (the one used before the Connect dialog existed).
pub const UNIVERSAL_ENV: &str = "ARQUS_API_KEY";

pub fn find(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id)
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeySource {
    Env(String), // the name of the variable
    Stored,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnStatus {
    Connected(KeySource),
    NoKeyNeeded,
    NotConnected,
}

/// Where does the key for this provider come from? Order: provider variable, ARQUS_API_KEY, saved key.
pub fn resolve_key(
    info: &ProviderInfo,
    env: &dyn Fn(&str) -> Option<String>,
    store: &AuthStore,
) -> Option<(String, KeySource)> {
    for name in info.env_vars {
        if let Some(v) = env(name).filter(|v| !v.trim().is_empty()) {
            return Some((v.trim().to_string(), KeySource::Env(name.to_string())));
        }
    }
    if info.needs_key {
        if let Some(v) = env(UNIVERSAL_ENV).filter(|v| !v.trim().is_empty()) {
            return Some((v.trim().to_string(), KeySource::Env(UNIVERSAL_ENV.to_string())));
        }
    }
    store.get_key(info.id).map(|k| (k.to_string(), KeySource::Stored))
}

pub fn status(info: &ProviderInfo, env: &dyn Fn(&str) -> Option<String>, store: &AuthStore) -> ConnStatus {
    match resolve_key(info, env, store) {
        Some((_, src)) => ConnStatus::Connected(src),
        None if !info.needs_key => ConnStatus::NoKeyNeeded,
        None => ConnStatus::NotConnected,
    }
}

/// URL that lists models. `tools_only` adds OpenRouter's server-side filter for tool-capable models.
pub fn models_url(info: &ProviderInfo, base_override: Option<&str>, tools_only: bool) -> String {
    let base = base_override.filter(|b| !b.is_empty()).unwrap_or(info.base_url).trim_end_matches('/');
    let mut url = format!("{base}/models");
    if tools_only && info.id == "openrouter" {
        url.push_str("?supported_parameters=tools");
    }
    url
}

/// URL used to check a key. OpenRouter's /models answers WITHOUT a key, so it uses /key (which also returns the key's limit).
pub fn validate_url(info: &ProviderInfo, base_override: Option<&str>) -> String {
    let base = base_override.filter(|b| !b.is_empty()).unwrap_or(info.base_url).trim_end_matches('/');
    format!("{base}{}", info.validate_path)
}

/// Header (name, value) pairs for a request to this provider. Header NAMES are lowercase.
pub fn auth_headers(info: &ProviderInfo, key: Option<&str>) -> Vec<(String, String)> {
    let mut h = Vec::new();
    match (info.kind, key) {
        (ApiKind::Anthropic, Some(k)) => {
            h.push(("x-api-key".to_string(), k.to_string()));
            h.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
        }
        (ApiKind::Anthropic, None) => h.push(("anthropic-version".to_string(), "2023-06-01".to_string())),
        (ApiKind::OpenAiCompat, Some(k)) => h.push(("authorization".to_string(), format!("Bearer {k}"))),
        (ApiKind::OpenAiCompat, None) => {}
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn ids_are_unique_lowercase_and_sane() {
        let mut seen = std::collections::HashSet::new();
        for p in PROVIDERS {
            assert!(seen.insert(p.id), "duplicate id {}", p.id);
            assert_eq!(p.id, p.id.to_lowercase());
            if p.needs_key {
                assert!(!p.env_vars.is_empty(), "{} needs a key but has no env var", p.id);
                assert!(p.key_url.starts_with("https://"));
            }
            if p.id != "custom" {
                assert!(p.base_url.starts_with("http"));
            }
        }
        assert!(find("openrouter").unwrap().recommended);
        assert!(find("nope").is_none());
    }

    #[test]
    fn key_resolution_order() {
        let or = find("openrouter").unwrap();
        let mut store = AuthStore::default();
        assert_eq!(status(or, &no_env, &store), ConnStatus::NotConnected);

        store.set_api_key("openrouter", "stored-key");
        assert_eq!(resolve_key(or, &no_env, &store).unwrap(), ("stored-key".into(), KeySource::Stored));

        let universal = |n: &str| if n == UNIVERSAL_ENV { Some("uni-key".to_string()) } else { None };
        assert_eq!(resolve_key(or, &universal, &store).unwrap().1, KeySource::Env(UNIVERSAL_ENV.into()));

        let specific = |n: &str| match n {
            "OPENROUTER_API_KEY" => Some(" env-key \n".to_string()),
            UNIVERSAL_ENV => Some("uni-key".to_string()),
            _ => None,
        };
        let (k, src) = resolve_key(or, &specific, &store).unwrap();
        assert_eq!(k, "env-key");
        assert_eq!(src, KeySource::Env("OPENROUTER_API_KEY".into()));

        let blank = |_: &str| Some("   ".to_string());
        assert_eq!(resolve_key(or, &blank, &AuthStore::default()), None);
    }

    #[test]
    fn local_providers_need_no_key() {
        let ol = find("ollama").unwrap();
        assert_eq!(status(ol, &no_env, &AuthStore::default()), ConnStatus::NoKeyNeeded);
        let universal = |n: &str| if n == UNIVERSAL_ENV { Some("uni".to_string()) } else { None };
        assert_eq!(resolve_key(ol, &universal, &AuthStore::default()), None); // universal key is NOT sent to local servers
    }

    #[test]
    fn urls_and_headers() {
        let or = find("openrouter").unwrap();
        assert_eq!(models_url(or, None, false), "https://openrouter.ai/api/v1/models");
        assert_eq!(models_url(or, None, true), "https://openrouter.ai/api/v1/models?supported_parameters=tools");
        let c = find("custom").unwrap();
        assert_eq!(models_url(c, Some("http://127.0.0.1:8080/v1/"), true), "http://127.0.0.1:8080/v1/models");
        assert_eq!(auth_headers(or, Some("k")), vec![("authorization".to_string(), "Bearer k".to_string())]);
        let an = find("anthropic").unwrap();
        let h = auth_headers(an, Some("k"));
        assert!(h.contains(&("x-api-key".to_string(), "k".to_string())));
        assert!(h.contains(&("anthropic-version".to_string(), "2023-06-01".to_string())));
        assert!(auth_headers(find("ollama").unwrap(), None).is_empty());
        assert_eq!(validate_url(or, None), "https://openrouter.ai/api/v1/key");
        assert_eq!(validate_url(an, None), "https://api.anthropic.com/v1/models");
        assert_eq!(validate_url(c, Some("http://h:1/v1/")), "http://h:1/v1/models");
    }
}
