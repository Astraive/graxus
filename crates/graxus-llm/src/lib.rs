//! Graxus LLM -- LLM provider abstraction for documentation generation.
//!
//! This crate provides a unified interface for interacting with multiple LLM providers
//! (OpenAI, Anthropic, Ollama) with built-in cost tracking, rate limiting, and retry logic.
//!
//! # Architecture
//!
//! - [`provider`] -- Core types (`LlmRequest`, `LlmResponse`, `LlmProvider` trait)
//! - [`providers`] -- Concrete provider implementations
//! - [`generator`] -- High-level documentation generation orchestration
//! - [`prompts`] -- Prompt templates for documentation tasks
//! - [`cost`] -- Token-based cost tracking and budget enforcement
//! - [`rate_limit`] -- Request rate limiting

pub mod cost;
pub mod generator;
pub mod prompts;
pub mod provider;
pub mod providers;
pub mod rate_limit;
