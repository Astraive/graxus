//! Token-based cost tracking and budget enforcement.
//!
//! The [`CostTracker`] accumulates token usage across multiple LLM calls
//! and enforces a configurable USD budget limit.

use serde::{Deserialize, Serialize};

/// Tracks cumulative API cost across multiple LLM calls.
///
/// Maintains running totals of input/output tokens and enforces a maximum
/// cost budget. If a call would exceed the budget, [`CostTracker::record`]
/// returns an error.
///
/// # Pricing
///
/// Pricing is per-million-tokens and varies by model. Unknown models
/// fall back to `gpt-4o-mini` pricing.
#[derive(Debug, Clone)]
pub struct CostTracker {
    /// Cumulative input tokens consumed.
    pub total_input_tokens: usize,
    /// Cumulative output tokens generated.
    pub total_output_tokens: usize,
    /// Total number of API requests tracked.
    pub total_requests: usize,
    /// Maximum allowed cost in USD before errors are raised.
    pub max_cost_usd: f64,
}

impl CostTracker {
    /// Create a new cost tracker with the given USD budget limit.
    pub fn new(max_cost_usd: f64) -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_requests: 0,
            max_cost_usd,
        }
    }

    /// Record token usage from a completed LLM call.
    ///
    /// Increments internal counters and checks whether the cumulative cost
    /// exceeds the configured budget.
    ///
    /// # Errors
    ///
    /// Returns an error if the estimated cumulative cost exceeds [`Self::max_cost_usd`].
    pub fn record(&mut self, input: usize, output: usize, model: &str) -> anyhow::Result<()> {
        self.total_input_tokens += input;
        self.total_output_tokens += output;
        self.total_requests += 1;
        let cost = self.estimate_cost_usd(model);
        if cost > self.max_cost_usd {
            anyhow::bail!(
                "Cost limit exceeded: ${:.4} > ${:.2}",
                cost,
                self.max_cost_usd
            );
        }
        Ok(())
    }

    /// Estimate the cumulative cost in USD for the given model.
    ///
    /// Pricing is per-million-tokens. Falls back to `gpt-4o-mini` pricing
    /// for unrecognized model names. To add pricing for new models, extend
    /// the match arms in this method.
    pub fn estimate_cost_usd(&self, model: &str) -> f64 {
        let (input_price, output_price) = Self::model_pricing(model);
        (self.total_input_tokens as f64 / 1_000_000.0 * input_price)
            + (self.total_output_tokens as f64 / 1_000_000.0 * output_price)
    }

    /// Look up per-million-token pricing for a model.
    ///
    /// Returns `(input_price_per_mtok, output_price_per_mtok)`.
    /// Unknown models fall back to `gpt-4o-mini` pricing.
    pub fn model_pricing(model: &str) -> (f64, f64) {
        match model {
            // OpenAI models
            "gpt-4o" => (2.50, 10.00),
            "gpt-4o-mini" => (0.15, 0.60),
            "gpt-4-turbo" => (10.00, 30.00),
            "gpt-3.5-turbo" => (0.50, 1.50),
            // Anthropic models
            "claude-3-5-sonnet" | "claude-3.5-sonnet" | "claude-sonnet-4-20250514" => {
                (3.00, 15.00)
            }
            "claude-3-opus" | "claude-3-opus-20240229" => (15.00, 75.00),
            "claude-3-haiku" | "claude-3-haiku-20240307" => (0.25, 1.25),
            // Fallback
            _ => (0.15, 0.60),
        }
    }

    /// Generate a snapshot summary of current cost and usage.
    pub fn summary(&self) -> CostSummary {
        CostSummary {
            total_input_tokens: self.total_input_tokens,
            total_output_tokens: self.total_output_tokens,
            total_requests: self.total_requests,
            estimated_usd: self.estimate_cost_usd("gpt-4o-mini"),
        }
    }
}

/// A snapshot of cumulative cost and usage data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    /// Total input tokens consumed.
    pub total_input_tokens: usize,
    /// Total output tokens generated.
    pub total_output_tokens: usize,
    /// Total number of API requests.
    pub total_requests: usize,
    /// Estimated total cost in USD (using gpt-4o-mini pricing).
    pub estimated_usd: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_starts_at_zero() {
        let tracker = CostTracker::new(10.0);
        assert_eq!(tracker.total_input_tokens, 0);
        assert_eq!(tracker.total_output_tokens, 0);
        assert_eq!(tracker.total_requests, 0);
        assert_eq!(tracker.max_cost_usd, 10.0);
    }

    #[test]
    fn record_accumulates_tokens() {
        let mut tracker = CostTracker::new(100.0);
        tracker.record(1000, 500, "gpt-4o-mini").unwrap();
        assert_eq!(tracker.total_input_tokens, 1000);
        assert_eq!(tracker.total_output_tokens, 500);
        assert_eq!(tracker.total_requests, 1);

        tracker.record(2000, 1000, "gpt-4o-mini").unwrap();
        assert_eq!(tracker.total_input_tokens, 3000);
        assert_eq!(tracker.total_output_tokens, 1500);
        assert_eq!(tracker.total_requests, 2);
    }

    #[test]
    fn record_exceeds_budget_returns_error() {
        let mut tracker = CostTracker::new(0.001); // Very low budget
                                                   // 1M input + 1M output at gpt-4o-mini pricing = $0.15 + $0.60 = $0.75
        let result = tracker.record(1_000_000, 1_000_000, "gpt-4o-mini");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cost limit exceeded"));
    }

    #[test]
    fn estimate_cost_gpt4o_pricing() {
        let mut tracker = CostTracker::new(100.0);
        tracker.total_input_tokens = 1_000_000;
        tracker.total_output_tokens = 1_000_000;
        let cost = tracker.estimate_cost_usd("gpt-4o");
        // $2.50 per 1M input + $10.00 per 1M output = $12.50
        assert!((cost - 12.50).abs() < 0.001);
    }

    #[test]
    fn estimate_cost_unknown_model_uses_default() {
        let mut tracker = CostTracker::new(100.0);
        tracker.total_input_tokens = 1_000_000;
        tracker.total_output_tokens = 0;
        let cost = tracker.estimate_cost_usd("some-unknown-model");
        // Falls back to gpt-4o-mini input: $0.15 per 1M
        assert!((cost - 0.15).abs() < 0.001);
    }

    #[test]
    fn estimate_cost_claude_pricing() {
        let mut tracker = CostTracker::new(100.0);
        tracker.total_input_tokens = 1_000_000;
        tracker.total_output_tokens = 1_000_000;
        let cost = tracker.estimate_cost_usd("claude-3-5-sonnet");
        // $3.00 per 1M input + $15.00 per 1M output = $18.00
        assert!((cost - 18.00).abs() < 0.001);
    }

    #[test]
    fn summary_uses_default_pricing() {
        let mut tracker = CostTracker::new(100.0);
        tracker.record(1000, 500, "gpt-4o").unwrap();
        let summary = tracker.summary();
        assert_eq!(summary.total_input_tokens, 1000);
        assert_eq!(summary.total_output_tokens, 500);
        assert_eq!(summary.total_requests, 1);
        // Summary always uses gpt-4o-mini pricing
        let expected = 1000.0 / 1_000_000.0 * 0.15 + 500.0 / 1_000_000.0 * 0.60;
        assert!((summary.estimated_usd - expected).abs() < 0.0001);
    }

    #[test]
    fn zero_usage_costs_nothing() {
        let tracker = CostTracker::new(10.0);
        assert!((tracker.estimate_cost_usd("gpt-4o") - 0.0).abs() < 0.0001);
    }
}
