//! Model Context Protocol (MCP) client support for arqus.

pub mod client;
pub mod config;
pub mod manager;
pub mod schema;
pub mod transport;
pub mod types;

pub use manager::McpManager;
pub use schema::{parse_tool_name, MCP_PREFIX};

#[cfg(test)]
mod adversarial_tests;
#[cfg(test)]
mod serena_tests;
#[cfg(test)]
mod tests;
