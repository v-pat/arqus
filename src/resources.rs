//! The customization layer: AGENTS.md, SYSTEM.md, skills and prompt templates.
//!
//! Strategy for the human:
//! - AGENTS.md: short, stable project facts: how to build, test and run; code
//!   style; folder map. Under about 100 lines, because it is sent on EVERY request.
//! - SYSTEM.md: only when you want a different persona or rules. It REPLACES the
//!   base prompt, so use APPEND_SYSTEM.md for small additions.
//! - Skills: procedures you need sometimes (release process, DB migration,
//!   writing a blog post). They cost almost nothing until used.
//! - Prompt templates: prompts you type often (/review, /commit-msg).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One AGENTS.md (or CLAUDE.md) file loaded into the system prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentFile {
    pub path: PathBuf,
    pub text: String,
}

/// One skill discovered from a `SKILL.md` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

/// Everything the system prompt and slash expansion need.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub agents: Vec<AgentFile>,
    pub system: Option<String>,
    pub append: Option<String>,
    pub skills: Vec<Skill>,
}

/// Load resources for `cwd`, using the real arqus home folder.
pub fn load(cwd: &Path) -> Resources {
    load_in(&crate::home::arqus_home(), cwd)
}

/// Load resources and scan extra skill folders from settings.
pub fn load_with(cwd: &Path, extra_skill_folders: &[String]) -> Resources {
    load_in_with(&crate::home::arqus_home(), cwd, extra_skill_folders)
}

/// Same as `load`, but with an explicit arqus home folder. Tests use this.
pub fn load_in(home: &Path, cwd: &Path) -> Resources {
    load_in_with(home, cwd, &[])
}

/// Same as `load_in`, plus extra skill folders. `home` is the arqus home, so
/// the global files live directly inside it.
pub fn load_in_with(home: &Path, cwd: &Path, extra_skill_folders: &[String]) -> Resources {
    Resources {
        home: home.to_path_buf(),
        cwd: cwd.to_path_buf(),
        agents: load_agents(home, cwd),
        system: read_first(&[cwd.join(".arqus").join("SYSTEM.md"), home.join("SYSTEM.md")]),
        append: read_first(&[
            cwd.join(".arqus").join("APPEND_SYSTEM.md"),
            home.join("APPEND_SYSTEM.md"),
        ]),
        skills: load_skills(home, cwd, extra_skill_folders),
    }
}

/// Global `<ARQUS_HOME>/AGENTS.md`, then each folder from the root down to cwd.
/// In each folder `AGENTS.md` wins; `CLAUDE.md` is the fallback.
fn load_agents(home: &Path, cwd: &Path) -> Vec<AgentFile> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    push_agent(&mut out, &mut seen, &home.join("AGENTS.md"));

    let mut dirs: Vec<PathBuf> = cwd.ancestors().map(|p| p.to_path_buf()).collect();
    dirs.reverse();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        let path = if dir.join("AGENTS.md").is_file() {
            dir.join("AGENTS.md")
        } else {
            dir.join("CLAUDE.md")
        };
        push_agent(&mut out, &mut seen, &path);
    }
    out
}

fn push_agent(out: &mut Vec<AgentFile>, seen: &mut HashSet<PathBuf>, path: &Path) {
    if !path.is_file() || !seen.insert(path.to_path_buf()) {
        return;
    }
    if let Ok(text) = fs::read_to_string(path) {
        out.push(AgentFile {
            path: path.to_path_buf(),
            text,
        });
    }
}

/// First readable file in the list.
fn read_first(paths: &[PathBuf]) -> Option<String> {
    paths.iter().find_map(|p| fs::read_to_string(p).ok())
}

/// Global skills first, then project skills, then folders listed in settings.
/// A later skill wins on a name clash.
fn load_skills(home: &Path, cwd: &Path, extra: &[String]) -> Vec<Skill> {
    let mut by_name: BTreeMap<String, Skill> = BTreeMap::new();
    let mut roots = vec![home.join("skills"), cwd.join(".arqus").join("skills")];
    roots.extend(extra.iter().map(|raw| resolve_folder(raw, home, cwd)));
    for root in roots {
        for skill in skills_in(&root) {
            by_name.insert(skill.name.clone(), skill);
        }
    }
    by_name.into_values().collect()
}

/// `~/x` is under the home folder, an absolute path stays, anything else is
/// relative to the project.
fn resolve_folder(raw: &str, home: &Path, cwd: &Path) -> PathBuf {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("~/") {
        return home.join(rest);
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn skills_in(root: &Path) -> Vec<Skill> {
    let mut dirs: Vec<PathBuf> = match fs::read_dir(root) {
        Ok(entries) => entries.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => return Vec::new(),
    };
    dirs.sort();
    dirs.iter()
        .filter_map(|dir| parse_skill(&dir.join("SKILL.md")))
        .collect()
}

/// Parse `SKILL.md`. Returns None when the frontmatter has no name or description.
pub fn parse_skill(path: &Path) -> Option<Skill> {
    let text = fs::read_to_string(path).ok()?;
    let (name, description) = parse_frontmatter(&text)?;
    Some(Skill {
        name,
        description,
        path: path.to_path_buf(),
    })
}

/// Tiny line parser for the frontmatter. Not a YAML implementation.
fn parse_frontmatter(text: &str) -> Option<(String, String)> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut name = None;
    let mut description = None;
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = unquote(value.trim());
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "name" => name = Some(value),
            "description" => description = Some(value),
            _ => {}
        }
    }
    Some((name?, description?))
}

fn unquote(text: &str) -> String {
    let bytes = text.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return text[1..text.len() - 1].to_string();
    }
    text.to_string()
}

/// Render the `<available_skills>` block. Max 50 skills, 200 chars each.
pub fn skills_block(skills: &[Skill]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }
    let mut out = String::from("<available_skills>\n");
    for skill in skills.iter().take(50) {
        let description: String = skill.description.chars().take(200).collect();
        out.push_str(&format!(
            "- {}: {} (file: {})\n",
            skill.name,
            description,
            skill.path.display()
        ));
    }
    out.push_str("</available_skills>\n");
    out.push_str("When a task matches a skill's description, first read that SKILL.md with the read tool, then follow it. Paths inside a skill are relative to the skill's folder.\n");
    Some(out)
}

/// Expand `/skill:name ...` and `/name ...` prompt templates.
/// Unknown slash words and all other text pass through unchanged.
pub fn expand_input(res: &Resources, input: &str) -> String {
    let trimmed = input.trim_start();
    let Some(body) = trimmed.strip_prefix('/') else {
        return input.to_string();
    };
    let end = body.find(char::is_whitespace).unwrap_or(body.len());
    let word = &body[..end];
    let rest = body[end..].trim_start();
    if word.is_empty() {
        return input.to_string();
    }

    if let Some(name) = word.strip_prefix("skill:") {
        return match res.skills.iter().find(|s| s.name == name) {
            Some(skill) => match fs::read_to_string(&skill.path) {
                Ok(text) => join_rest(&text, rest),
                Err(_) => input.to_string(),
            },
            None => input.to_string(),
        };
    }

    if !valid_name(word) {
        return input.to_string();
    }
    match read_prompt(res, word) {
        Some(text) => text.replace("$ARGUMENTS", rest),
        None => input.to_string(),
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn read_prompt(res: &Resources, name: &str) -> Option<String> {
    let file = format!("{name}.md");
    read_first(&[
        res.cwd.join(".arqus").join("prompts").join(&file),
        res.home.join("prompts").join(&file),
    ])
}

fn join_rest(text: &str, rest: &str) -> String {
    if rest.is_empty() {
        text.to_string()
    } else {
        format!("{}\n\n{}", text.trim_end(), rest)
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
