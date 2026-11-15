use serde::de::DeserializeOwned;

use crate::flow::errors::FlowError;

pub fn strip_think_tags(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut result = String::with_capacity(text.len());
    let mut idx = 0;

    while idx < text.len() {
        if let Some(open_rel) = lower[idx..].find("<think>") {
            let open_pos = idx + open_rel;
            result.push_str(&text[idx..open_pos]);
            if let Some(close_rel) = lower[open_pos..].find("</think>") {
                let close_pos = open_pos + close_rel + "</think>".len();
                idx = close_pos;
            } else {
                return result;
            }
        } else {
            result.push_str(&text[idx..]);
            break;
        }
    }

    result
}

pub fn extract_json_text(raw: &str) -> Result<String, FlowError> {
    let text = strip_think_tags(raw);

    if let Some(start_fence) = text.find("```") {
        let after_fence = &text[start_fence + 3..];
        if let Some(newline_pos) = after_fence.find('\n') {
            let content_start = start_fence + 3 + newline_pos + 1;
            if let Some(end_fence) = text[content_start..].find("```") {
                let candidate = text[content_start..content_start + end_fence].trim();
                if candidate.starts_with('[') {
                    return Err(FlowError::ParseError(
                        "expected a JSON object, found an array".to_string(),
                    ));
                }
                if candidate.starts_with('{') {
                    return Ok(candidate.to_string());
                }
            }
        }
    }

    let trimmed = text.trim();
    if trimmed.starts_with('[') {
        return Err(FlowError::ParseError(
            "expected a JSON object, found an array".to_string(),
        ));
    }

    let mut depth: u32 = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut start_pos: Option<usize> = None;

    for (idx, ch) in text.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else {
            match ch {
                '"' => in_string = true,
                '[' if depth == 0 && start_pos.is_none() => {
                    return Err(FlowError::ParseError(
                        "expected a JSON object, found an array".to_string(),
                    ));
                }
                '{' => {
                    if depth == 0 {
                        start_pos = Some(idx);
                    }
                    depth = depth.saturating_add(1);
                }
                '}' if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(start) = start_pos {
                            let slice = &text[start..idx + ch.len_utf8()];
                            return Ok(slice.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Err(FlowError::ParseError(
        "no valid JSON object found in response".to_string(),
    ))
}

fn extract_last_json_key(slice: &str) -> Option<String> {
    let bytes = slice.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b':' {
            let before = slice[..i].trim_end();
            if let Some(end_quote) = before.rfind('"') {
                if let Some(start_quote) = before[..end_quote].rfind('"') {
                    let key = &before[start_quote + 1..end_quote];
                    if !key.is_empty() {
                        return Some(key.to_string());
                    }
                }
            }
        }
    }
    None
}

fn find_key_near_location(raw: &str, target_line: usize, target_col: usize) -> Option<String> {
    let mut current_line = 1;
    let mut line_start = 0;

    for (idx, ch) in raw.char_indices() {
        if current_line == target_line {
            let end = (line_start + target_col).min(raw.len());
            let line_slice = &raw[line_start..end];
            return extract_last_json_key(line_slice);
        }
        if ch == '\n' {
            current_line += 1;
            line_start = idx + 1;
        }
    }
    extract_last_json_key(&raw[line_start..])
}

pub fn serde_repair_message(err: &serde_json::Error, raw_json: &str) -> String {
    let err_str = err.to_string();

    let mut backticked = None;
    if let Some(start) = err_str.find('`') {
        if let Some(end) = err_str[start + 1..].find('`') {
            backticked = Some(&err_str[start + 1..start + 1 + end]);
        }
    }

    let line = err.line();
    let col = err.column();

    if let Some(field) = backticked {
        if err_str.contains("missing field") {
            return format!(
                "missing field '{field}' at line {line} column {col}. Please provide this required field in the JSON output."
            );
        }
        if err_str.contains("unknown field") {
            return format!(
                "unknown field '{field}' at line {line} column {col}. This field is not defined in the schema."
            );
        }
        return format!("error at field '{field}' (line {line} column {col}): {err_str}");
    }

    if let Some(field) = find_key_near_location(raw_json, line, col) {
        format!("schema type mismatch at field '{field}' (line {line} column {col}): {err_str}")
    } else {
        format!("JSON deserialization error at line {line} column {col}: {err_str}")
    }
}

pub fn parse_json_object<T: DeserializeOwned>(text: &str) -> Result<T, FlowError> {
    let extracted = extract_json_text(text)?;
    match serde_json::from_str::<T>(&extracted) {
        Ok(val) => Ok(val),
        Err(err) => {
            let repair = serde_repair_message(&err, &extracted);
            Err(FlowError::ParseError(repair))
        }
    }
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
