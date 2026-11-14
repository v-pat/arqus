pub mod types;
pub use types::*;

pub fn validate_task_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes[0] != b'T' {
        return false;
    }
    let rest = &bytes[1..];
    let dot_pos = rest.iter().position(|&b| b == b'.');
    let base = match dot_pos {
        Some(pos) => &rest[..pos],
        None => rest,
    };
    if base.len() < 2 || base.len() > 3 || !base.iter().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if let Some(pos) = dot_pos {
        for (i, seg) in rest[pos..].split(|&b| b == b'.').enumerate() {
            if (i > 0 && (seg.is_empty() || !seg.iter().all(|b| b.is_ascii_digit())))
                || (i == 0 && !seg.is_empty())
            {
                return false;
            }
        }
    }
    true
}

pub fn validate_file_ref(file: &FileRef) -> Vec<Issue> {
    let mut out = Vec::new();
    match (file.start_line, file.end_line) {
        (Some(s), Some(e)) if s > e => out.push(Issue {
            task_id: None,
            code: IssueCode::InvalidLineRange,
            message: format!("start_line ({s}) > end_line ({e}) in {}", file.path),
        }),
        (Some(_), None) | (None, Some(_)) => out.push(Issue {
            task_id: None,
            code: IssueCode::InvalidLineRange,
            message: format!("line range in {} must specify both ends", file.path),
        }),
        _ => {}
    }
    out
}

struct ShapeChecker {
    issues: Vec<Issue>,
    id_opt: Option<String>,
}

impl ShapeChecker {
    fn add(&mut self, code: IssueCode, msg: String) {
        self.issues.push(Issue {
            task_id: self.id_opt.clone(),
            code,
            message: msg,
        });
    }

    fn check_max(&mut self, code: IssueCode, name: &str, text: &str, max: usize) {
        let n = text.chars().count();
        if n > max {
            self.add(code, format!("{name} exceeds {max} chars ({n})"));
        }
    }
}

pub fn validate_shape(task: &TaskContract) -> Vec<Issue> {
    let mut c = ShapeChecker {
        issues: Vec::new(),
        id_opt: Some(task.id.clone()),
    };
    if !validate_task_id(&task.id) {
        c.add(
            IssueCode::InvalidId,
            format!("task id '{}' invalid", task.id),
        );
    }
    c.check_max(IssueCode::TitleTooLong, "title", &task.title, 80);
    c.check_max(
        IssueCode::ObjectiveTooLong,
        "objective",
        &task.objective,
        600,
    );
    if task.constraints.len() > 6 {
        c.add(
            IssueCode::ConstraintsTooMany,
            format!("constraints > 6 ({})", task.constraints.len()),
        );
    }
    for (i, item) in task.constraints.iter().enumerate() {
        c.check_max(
            IssueCode::ConstraintTooLong,
            &format!("constraint {i}"),
            item,
            200,
        );
    }
    if task.acceptance_criteria.len() > 6 {
        c.add(
            IssueCode::AcceptanceCriteriaTooMany,
            format!("criteria > 6 ({})", task.acceptance_criteria.len()),
        );
    }
    c.check_max(
        IssueCode::InterfaceNotesTooLong,
        "notes",
        &task.interface_notes,
        800,
    );
    for file in &task.context {
        for mut issue in validate_file_ref(file) {
            issue.task_id = c.id_opt.clone();
            c.issues.push(issue);
        }
    }
    c.issues
}

#[cfg(test)]
mod tests;
