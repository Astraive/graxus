//! Graxus agent API — Docs-code bridge and context queries for AI agents.

pub mod bridge;
pub mod context;
pub mod diff_context;
pub mod export;

pub use bridge::{BridgeBuilder, BridgeEdge, BridgeEdgeType};
pub use context::{
    estimate_tokens, ContextBudget, ContextEngine, ContextQueryType, Priority, ScoredItem,
};
pub use diff_context::{build_diff_context, parse_diff_paths, DiffContext};
pub use export::AgentExport;
