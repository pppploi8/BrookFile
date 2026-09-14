pub mod client;
pub mod compaction;
pub mod provider;
pub mod store;
pub mod tools;

pub use client::{AiCallConfig, ToolCallDone};
pub use provider::{preset_by_type, PROVIDER_PRESETS};
pub use store::{ChatFile, ChatIndex, ChatMeta, StoredMessage};
pub use tools::{AiToolContext, AiToolProvider, AiToolRegistry, ToolDef, ToolResult};
