//! Graxus agent API — Docs-code bridge and context queries for AI agents.

pub mod bridge;
pub mod context;
pub mod diff_context;
pub mod export;

pub use bridge::{BridgeBuilder, BridgeEdge, BridgeEdgeType};
pub use context::{ContextBudget, ContextEngine, ContextQueryType, Priority, ScoredItem, estimate_tokens};
pub use diff_context::{DiffContext, build_diff_context, parse_diff_paths};
pub use export::AgentExport;
