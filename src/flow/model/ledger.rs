use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::flow::config::types::PriceConfig;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Planner,
    Redesign,
    Takeover,
    Worker,
}

impl Role {
    pub fn is_cloud(&self) -> bool {
        matches!(self, Role::Planner | Role::Redesign | Role::Takeover)
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Role::Worker)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct UsageRecord {
    pub ts: u64,
    pub run_id: String,
    pub task_id: Option<String>,
    pub role: Role,
    pub provider: String,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub estimated: bool,
    pub cost_usd: Option<f64>,
    pub latency_ms: u64,
    pub ok: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RoleTotals {
    pub requests: u64,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost_usd: Option<f64>,
}

impl RoleTotals {
    pub fn add_record(&mut self, rec: &UsageRecord) {
        self.requests = self.requests.saturating_add(1);
        self.input = self.input.saturating_add(rec.input);
        self.output = self.output.saturating_add(rec.output);
        self.cache_read = self.cache_read.saturating_add(rec.cache_read);
        self.cache_write = self.cache_write.saturating_add(rec.cache_write);
        if let Some(cost) = rec.cost_usd {
            self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + cost);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LedgerTotals {
    pub by_role: BTreeMap<Role, RoleTotals>,
    pub cloud: RoleTotals,
    pub local: RoleTotals,
    pub total: RoleTotals,
}

pub fn calculate_cost(
    reported_cost: Option<f64>,
    price: Option<&PriceConfig>,
    input: u64,
    output: u64,
    cache_read: u64,
) -> Option<f64> {
    if let Some(cost) = reported_cost {
        return Some(cost);
    }
    if let Some(price) = price {
        let input_cost = (input as f64) * price.input / 1_000_000.0;
        let output_cost = (output as f64) * price.output / 1_000_000.0;
        let cached_rate = price.cached_input.unwrap_or(price.input);
        let cached_cost = (cache_read as f64) * cached_rate / 1_000_000.0;
        return Some(input_cost + output_cost + cached_cost);
    }
    None
}

pub struct Ledger {
    path: Option<PathBuf>,
    records: Vec<UsageRecord>,
}

impl Ledger {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            records: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let mut records = Vec::new();
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let record: UsageRecord = serde_json::from_str(trimmed).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    })?;
                    records.push(record);
                }
            }
        }
        Ok(Self {
            path: Some(path.to_path_buf()),
            records,
        })
    }

    pub fn record(&mut self, record: UsageRecord) -> Result<(), std::io::Error> {
        if let Some(ref path) = self.path {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            let mut line = serde_json::to_string(&record).map_err(|err| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string())
            })?;
            line.push('\n');
            file.write_all(line.as_bytes())?;
            file.flush()?;
        }
        self.records.push(record);
        Ok(())
    }

    pub fn records(&self) -> &[UsageRecord] {
        &self.records
    }

    pub fn totals(&self) -> LedgerTotals {
        let mut by_role = BTreeMap::new();
        let mut cloud = RoleTotals::default();
        let mut local = RoleTotals::default();
        let mut total = RoleTotals::default();

        for rec in &self.records {
            let entry = by_role.entry(rec.role).or_insert_with(RoleTotals::default);
            entry.add_record(rec);

            if rec.role.is_cloud() {
                cloud.add_record(rec);
            } else {
                local.add_record(rec);
            }
            total.add_record(rec);
        }

        LedgerTotals {
            by_role,
            cloud,
            local,
            total,
        }
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
