//! Builds the system prompt string.
//!
//! Layers: base prompt, AGENTS.md files, skills list, date and cwd.
//! `SYSTEM.md` replaces the base prompt; `APPEND_SYSTEM.md` appends to it.

use crate::resources::{skills_block, Resources};

/// Small, provider-neutral core prompt. Keep it under 250 words.
pub const BASE_PROMPT: &str = "You are pirs, a coding assistant running in the user's terminal. You help by reading files, running commands, editing code and writing files.
Tools: read, write, edit, bash (see each tool's description).
- Look before you change: read a file before you edit it.
- Prefer edit (small exact replacements) over write for existing files.
- Use bash for searching (grep, find, ls), running tests, and git.
- Keep answers short and concrete. Show file paths clearly.
- When you finish, say what you changed and how to check it.";

/// Assemble: base (or SYSTEM.md) + APPEND_SYSTEM.md + AGENTS.md blocks +
/// skills block + current directory and date.
pub fn build_system_prompt(resources: &Resources, cwd: &str, date: &str) -> String {
    let mut out = String::new();
    match &resources.system {
        Some(text) => out.push_str(text.trim_end()),
        None => out.push_str(BASE_PROMPT),
    }
    out.push('\n');

    if let Some(text) = &resources.append {
        out.push('\n');
        out.push_str(text.trim_end());
        out.push('\n');
    }

    for file in &resources.agents {
        out.push_str(&format!(
            "\n<project-context path=\"{}\">\n{}\n</project-context>\n",
            file.path.display(),
            file.text.trim()
        ));
    }

    if let Some(block) = skills_block(&resources.skills) {
        out.push('\n');
        out.push_str(&block);
    }

    out.push_str(&format!("Current directory: {cwd}\nCurrent date: {date}\n"));
    out
}

/// Today in UTC as `YYYY-MM-DD`. No date crate needed: convert days since 1970.
pub fn today_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{AgentFile, Skill};
    use std::path::PathBuf;

    fn resources() -> Resources {
        Resources {
            home: PathBuf::from("/home/me"),
            cwd: PathBuf::from("/home/me/proj"),
            agents: Vec::new(),
            system: None,
            append: None,
            skills: Vec::new(),
        }
    }

    #[test]
    fn prompt_contains_cwd_and_is_short() {
        let p = build_system_prompt(&resources(), "/home/me/proj", "2026-11-06");
        assert!(p.contains("/home/me/proj"));
        assert!(p.contains("2026-11-06"));
        assert!(p.len() < 1500, "prompt is {} chars", p.len());
    }

    #[test]
    fn prompt_orders_context_then_skills_then_cwd() {
        let mut res = resources();
        res.agents.push(AgentFile {
            path: PathBuf::from("/home/me/proj/AGENTS.md"),
            text: "PROJECT RULES".to_string(),
        });
        res.skills.push(Skill {
            name: "demo".to_string(),
            description: "use for demos".to_string(),
            path: PathBuf::from("/home/me/proj/.pirs/skills/demo/SKILL.md"),
        });

        let p = build_system_prompt(&res, "/home/me/proj", "2026-11-06");
        let context = p.find("<project-context path=\"/home/me/proj/AGENTS.md\">");
        let skills = p.find("<available_skills>");
        let cwd = p.find("Current directory:");
        assert!(context.unwrap() < skills.unwrap());
        assert!(skills.unwrap() < cwd.unwrap());
        assert!(p.contains("PROJECT RULES"));
        assert!(p.contains("- demo: use for demos"));
    }

    #[test]
    fn no_skills_means_no_skills_block() {
        let p = build_system_prompt(&resources(), "/home/me/proj", "2026-11-06");
        assert!(!p.contains("<available_skills>"));
    }

    #[test]
    fn today_is_iso_shaped() {
        let today = today_utc();
        assert_eq!(today.len(), 10, "got {today}");
        assert_eq!(&today[4..5], "-");
        assert_eq!(&today[7..8], "-");
    }
}
