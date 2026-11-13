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
            assert!(
                !p.env_vars.is_empty(),
                "{} needs a key but has no env var",
                p.id
            );
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
    assert_eq!(
        resolve_key(or, &no_env, &store).unwrap(),
        ("stored-key".into(), KeySource::Stored)
    );

    let universal = |n: &str| {
        if n == UNIVERSAL_ENV {
            Some("uni-key".to_string())
        } else {
            None
        }
    };
    assert_eq!(
        resolve_key(or, &universal, &store).unwrap().1,
        KeySource::Env(UNIVERSAL_ENV.into())
    );

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
    assert_eq!(
        status(ol, &no_env, &AuthStore::default()),
        ConnStatus::NoKeyNeeded
    );
    let universal = |n: &str| {
        if n == UNIVERSAL_ENV {
            Some("uni".to_string())
        } else {
            None
        }
    };
    assert_eq!(resolve_key(ol, &universal, &AuthStore::default()), None); // universal key is NOT sent to local servers
}

#[test]
fn urls_and_headers() {
    let or = find("openrouter").unwrap();
    assert_eq!(
        models_url(or, None, false),
        "https://openrouter.ai/api/v1/models"
    );
    assert_eq!(
        models_url(or, None, true),
        "https://openrouter.ai/api/v1/models?supported_parameters=tools"
    );
    let c = find("custom").unwrap();
    assert_eq!(
        models_url(c, Some("http://127.0.0.1:8080/v1/"), true),
        "http://127.0.0.1:8080/v1/models"
    );
    assert_eq!(
        auth_headers(or, Some("k")),
        vec![("authorization".to_string(), "Bearer k".to_string())]
    );
    let an = find("anthropic").unwrap();
    let h = auth_headers(an, Some("k"));
    assert!(h.contains(&("x-api-key".to_string(), "k".to_string())));
    assert!(h.contains(&("anthropic-version".to_string(), "2023-06-01".to_string())));
    assert!(auth_headers(find("ollama").unwrap(), None).is_empty());
    assert_eq!(validate_url(or, None), "https://openrouter.ai/api/v1/key");
    assert_eq!(
        validate_url(an, None),
        "https://api.anthropic.com/v1/models"
    );
    assert_eq!(
        validate_url(c, Some("http://h:1/v1/")),
        "http://h:1/v1/models"
    );
}
