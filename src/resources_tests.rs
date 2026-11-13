use super::*;
use crate::system_prompt::build_system_prompt;

fn tmp() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("arqus_res_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

fn texts(files: &[AgentFile]) -> Vec<String> {
    files.iter().map(|f| f.text.clone()).collect()
}

#[test]
fn agents_global_then_root_to_cwd() {
    let home = tmp();
    let project = tmp();
    let sub = project.join("sub");
    fs::create_dir_all(&sub).unwrap();
    write(&home.join("AGENTS.md"), "global");
    write(&project.join("AGENTS.md"), "outer");
    write(&sub.join("AGENTS.md"), "inner");

    let res = load_in(&home, &sub);
    assert_eq!(texts(&res.agents), vec!["global", "outer", "inner"]);
}

#[test]
fn claude_md_is_the_fallback() {
    let home = tmp();
    let project = tmp();
    let sub = project.join("sub");
    fs::create_dir_all(&sub).unwrap();
    write(&project.join("CLAUDE.md"), "outer-claude");
    write(&project.join("AGENTS.md"), "outer-agents");
    write(&sub.join("CLAUDE.md"), "inner-claude");

    let res = load_in(&home, &sub);
    assert_eq!(texts(&res.agents), vec!["outer-agents", "inner-claude"]);
}

#[test]
fn system_md_replaces_and_append_adds() {
    let home = tmp();
    let project = tmp();
    write(&project.join(".arqus/SYSTEM.md"), "CUSTOM PERSONA");
    write(&project.join(".arqus/APPEND_SYSTEM.md"), "EXTRA RULES");
    write(&home.join("SYSTEM.md"), "GLOBAL PERSONA");

    let res = load_in(&home, &project);
    let prompt = build_system_prompt(&res, "/proj", "2026-11-06");
    assert!(prompt.starts_with("CUSTOM PERSONA"));
    assert!(prompt.contains("EXTRA RULES"));
    assert!(!prompt.contains(crate::system_prompt::BASE_PROMPT));
    assert!(!prompt.contains("GLOBAL PERSONA"));
}

#[test]
fn global_system_md_is_the_fallback() {
    let home = tmp();
    let project = tmp();
    write(&home.join("SYSTEM.md"), "GLOBAL PERSONA");

    let res = load_in(&home, &project);
    let prompt = build_system_prompt(&res, "/proj", "2026-11-06");
    assert!(prompt.starts_with("GLOBAL PERSONA"));
    assert!(!prompt.contains(crate::system_prompt::BASE_PROMPT));
}

#[test]
fn project_skill_overrides_global_skill() {
    let home = tmp();
    let project = tmp();
    write(
        &home.join("skills/demo/SKILL.md"),
        "---\nname: demo\ndescription: global demo\n---\n\nglobal body\n",
    );
    write(
        &project.join(".arqus/skills/demo/SKILL.md"),
        "---\nname: demo\ndescription: project demo\n---\n\n# Demo\n\nproject body\n",
    );
    write(
        &home.join("skills/other/SKILL.md"),
        "---\nname: other\ndescription: other skill\n---\n\nbody\n",
    );

    let res = load_in(&home, &project);
    assert_eq!(res.skills.len(), 2);
    assert_eq!(res.skills[0].name, "demo");
    assert_eq!(res.skills[0].description, "project demo");
    assert!(res.skills[0].path.starts_with(&project));
    assert_eq!(res.skills[1].name, "other");
}

#[test]
fn broken_skills_are_skipped() {
    let home = tmp();
    let project = tmp();
    write(
        &home.join("skills/no-desc/SKILL.md"),
        "---\nname: no-desc\n---\n\nbody\n",
    );
    write(
        &home.join("skills/no-frontmatter/SKILL.md"),
        "# Just a title\n",
    );
    write(
        &home.join("skills/no-name/SKILL.md"),
        "---\ndescription: only a description\n---\n",
    );

    let res = load_in(&home, &project);
    assert!(res.skills.is_empty(), "got {:?}", res.skills);
}

#[test]
fn skill_block_lists_name_description_and_path() {
    let home = tmp();
    let project = tmp();
    write(
        &home.join("skills/demo/SKILL.md"),
        "---\nname: demo\ndescription: use for demos\n---\n\nbody\n",
    );

    let res = load_in(&home, &project);
    let block = skills_block(&res.skills).unwrap();
    assert!(block.contains("<available_skills>"));
    assert!(block.contains("- demo: use for demos (file: "));
    assert!(block.contains("SKILL.md"));
    assert!(block.contains("first read that SKILL.md"));
}

#[test]
fn skills_block_caps_count_and_description_length() {
    let long: String = "x".repeat(250);
    let skills: Vec<Skill> = (0..55)
        .map(|i| Skill {
            name: format!("s{i:02}"),
            description: long.clone(),
            path: PathBuf::from("/skills/SKILL.md"),
        })
        .collect();

    let block = skills_block(&skills).unwrap();
    assert!(block.contains("- s00: "));
    assert!(block.contains("- s49: "));
    assert!(!block.contains("- s50: "));
    assert!(block.contains(&"x".repeat(200)));
    assert!(!block.contains(&"x".repeat(201)));
}

#[test]
fn skill_slash_command_inlines_the_skill() {
    let home = tmp();
    let project = tmp();
    write(
        &home.join("skills/demo/SKILL.md"),
        "---\nname: demo\ndescription: use for demos\n---\n\n# Demo\n\nFollow these steps.\n",
    );

    let res = load_in(&home, &project);
    let out = expand_input(&res, "/skill:demo now please");
    assert!(out.contains("# Demo"));
    assert!(out.contains("Follow these steps."));
    assert!(out.ends_with("now please"));

    assert_eq!(expand_input(&res, "/skill:missing x"), "/skill:missing x");
}

#[test]
fn prompt_slash_command_replaces_arguments() {
    let home = tmp();
    let project = tmp();
    write(
        &project.join(".arqus/prompts/review.md"),
        "Review this: $ARGUMENTS\nThanks.",
    );
    write(&home.join("prompts/global.md"), "Global: $ARGUMENTS");

    let res = load_in(&home, &project);
    assert_eq!(
        expand_input(&res, "/review src/a.rs src/b.rs"),
        "Review this: src/a.rs src/b.rs\nThanks."
    );
    assert_eq!(expand_input(&res, "/global hi"), "Global: hi");
    assert_eq!(expand_input(&res, "/review"), "Review this: \nThanks.");
}

#[test]
fn unknown_slash_and_plain_text_pass_through() {
    let home = tmp();
    let project = tmp();
    let res = load_in(&home, &project);

    assert_eq!(expand_input(&res, "/nope rest"), "/nope rest");
    assert_eq!(expand_input(&res, "/../secret"), "/../secret");
    assert_eq!(expand_input(&res, "plain text"), "plain text");
    assert_eq!(expand_input(&res, "/"), "/");
}

#[test]
fn settings_skill_folders_are_scanned() {
    let home = tmp();
    let project = tmp();
    write(
        &project.join("skills/extra/SKILL.md"),
        "---\nname: extra\ndescription: from settings\n---\n",
    );
    write(
        &home.join("shared/another/SKILL.md"),
        "---\nname: another\ndescription: from home\n---\n",
    );

    let folders = vec!["skills".to_string(), "~/shared".to_string()];
    let res = load_in_with(&home, &project, &folders);
    let names: Vec<String> = res.skills.iter().map(|s| s.name.clone()).collect();
    assert_eq!(names, vec!["another", "extra"]);
}

#[test]
fn empty_skills_folder_is_not_an_error() {
    let home = tmp();
    let project = tmp();
    fs::create_dir_all(home.join("skills")).unwrap();
    fs::create_dir_all(project.join(".arqus/skills")).unwrap();

    let res = load_in(&home, &project);
    assert!(res.skills.is_empty());
    assert!(skills_block(&res.skills).is_none());
}

#[test]
fn prompt_template_with_a_bad_name_never_crashes() {
    let home = tmp();
    let project = tmp();
    write(&project.join(".arqus/prompts/bad name!.md"), "never loaded");

    let res = load_in(&home, &project);
    assert_eq!(expand_input(&res, "/bad name!"), "/bad name!");
    assert_eq!(expand_input(&res, "/bad name! rest"), "/bad name! rest");
    assert_eq!(expand_input(&res, "/../bad"), "/../bad");
    assert_eq!(expand_input(&res, "/"), "/");
}
