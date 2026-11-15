use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::prompts::{
    build_objective_message, build_planner_system_prompt, build_repo_map_message,
    format_json_repair_message, format_repair_message, format_requested_files,
};
use super::validator::validate_plan;
use crate::ai::{Llm, LlmRequest};
use crate::flow::config::types::ResolvedFlowConfig;
use crate::flow::errors::FlowError;
use crate::flow::model::complete::complete_text;
use crate::flow::model::json::parse_json_object;
use crate::flow::model::ledger::{calculate_cost, Ledger, Role, UsageRecord};
use crate::flow::schema::types::{FileRef, Plan, PlannerReply};
use crate::types::{Block, Message, StopReason};

pub struct Planner<'a> {
    pub llm: &'a Llm,
    pub config: &'a ResolvedFlowConfig,
    pub repo_path: &'a Path,
    pub base_files: &'a [String],
    pub repo_map: &'a str,
    pub capability_sheet: &'a str,
    pub ledger: &'a mut Ledger,
    pub run_id: &'a str,
}

impl<'a> Planner<'a> {
    pub async fn plan(&mut self, objective: &str) -> Result<Plan, FlowError> {
        let system = build_planner_system_prompt(self.capability_sheet);
        let mut messages = vec![
            build_repo_map_message(self.repo_map),
            build_objective_message(objective),
        ];

        let mut need_files_count: u32 = 0;
        let mut repairs_count: u32 = 0;

        loop {
            let req = LlmRequest {
                system: system.clone(),
                messages: messages.clone(),
                tools: vec![],
                max_tokens: self.config.orchestrator.max_output_tokens,
            };

            let completion = complete_text(self.llm, req).await?;

            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            let cost_usd = calculate_cost(
                completion.usage.cost,
                self.config.orchestrator.price.as_ref(),
                completion.usage.input,
                completion.usage.output,
                completion.usage.cache_read,
            );

            let record = UsageRecord {
                ts,
                run_id: self.run_id.to_string(),
                task_id: None,
                role: Role::Planner,
                provider: self.config.orchestrator.provider.clone(),
                model: self.config.orchestrator.model.clone(),
                input: completion.usage.input,
                output: completion.usage.output,
                cache_read: completion.usage.cache_read,
                cache_write: completion.usage.cache_write,
                estimated: completion.estimated,
                cost_usd,
                latency_ms: completion.latency_ms,
                ok: true,
            };
            let _ = self.ledger.record(record);

            let assistant_msg = Message::Assistant {
                blocks: vec![Block::Text {
                    text: completion.text.clone(),
                }],
                provider: self.config.orchestrator.provider.clone(),
                model: self.config.orchestrator.model.clone(),
                usage: Some(completion.usage.clone()),
                stop: StopReason::Stop,
            };

            match parse_json_object::<PlannerReply>(&completion.text) {
                Ok(PlannerReply::NeedFiles { files }) => {
                    if need_files_count < self.config.limits.max_file_read_rounds {
                        need_files_count += 1;
                        let content = self.read_requested_files(&files);
                        let formatted = format_requested_files(&content, 60 * 1024);
                        messages.push(assistant_msg);
                        messages.push(Message::User {
                            text: format!("### REQUESTED FILES\n\n{formatted}"),
                        });
                    } else {
                        messages.push(assistant_msg);
                        messages.push(Message::User {
                            text: "You have reached the maximum number of file read rounds. Please produce a complete Plan now conforming to the plan schema."
                                .to_string(),
                        });
                    }
                }
                Ok(PlannerReply::Plan {
                    summary,
                    tasks,
                    final_verify,
                }) => {
                    let plan = Plan {
                        summary,
                        tasks,
                        final_verify,
                    };
                    let issues =
                        validate_plan(&plan, self.base_files, self.config, Some(self.repo_path));
                    if issues.is_empty() {
                        return Ok(plan);
                    }
                    if repairs_count >= self.config.limits.max_planning_repairs {
                        return Err(FlowError::PlanningFailed(format!(
                            "Planning repair budget exhausted ({} repairs attempted). Issues: {:?}",
                            repairs_count, issues
                        )));
                    }
                    repairs_count += 1;
                    let repair_prompt = format_repair_message(&issues);
                    messages.push(assistant_msg);
                    messages.push(Message::User {
                        text: repair_prompt,
                    });
                }
                Err(FlowError::ParseError(msg)) => {
                    if repairs_count >= self.config.limits.max_planning_repairs {
                        return Err(FlowError::PlanningFailed(format!(
                            "Planning repair budget exhausted ({} repairs attempted). Parse error: {}",
                            repairs_count, msg
                        )));
                    }
                    repairs_count += 1;
                    let repair_prompt = format_json_repair_message(&msg);
                    messages.push(assistant_msg);
                    messages.push(Message::User {
                        text: repair_prompt,
                    });
                }
                Err(err) => return Err(err),
            }
        }
    }

    fn read_requested_files(&self, files: &[FileRef]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for file_ref in files.iter().take(8) {
            let full_path = self.repo_path.join(&file_ref.path);
            if let Ok(raw) = fs::read_to_string(&full_path) {
                let normalized = raw.replace("\r\n", "\n");
                let content = match (file_ref.start_line, file_ref.end_line) {
                    (Some(start), Some(end)) => {
                        let start_idx = (start.max(1) - 1) as usize;
                        let end_idx = end as usize;
                        let lines: Vec<&str> = normalized
                            .lines()
                            .skip(start_idx)
                            .take(end_idx.saturating_sub(start_idx))
                            .collect();
                        lines.join("\n")
                    }
                    _ => normalized,
                };
                out.push((file_ref.path.clone(), content));
            } else {
                out.push((
                    file_ref.path.clone(),
                    format!("(File '{}' could not be read)", file_ref.path),
                ));
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "planner_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "planner_repair_tests.rs"]
mod repair_tests;
