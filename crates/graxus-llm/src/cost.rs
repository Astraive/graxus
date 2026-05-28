use serde::{Deserialize, Serialize};

/// Tracks API cost across multiple LLM calls.
#[derive(Debug, Clone)]
pub struct CostTracker {
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_requests: usize,
    pub max_cost_usd: f64,
}

impl CostTracker {
    pub fn new(max_cost_usd: f64) -> Self {
        Self { total_input_tokens: 0, total_output_tokens: 0, total_requests: 0, max_cost_usd }
    }

    pub fn record(&mut self, input: usize, output: usize, model: &str) -> anyhow::Result<()> {
        self.total_input_tokens += input;
        self.total_output_tokens += output;
        self.total_requests += 1;
        let cost = self.estimate_cost_usd(model);
        if cost > self.max_cost_usd {
            anyhow::bail!("Cost limit exceeded: ${:.4} > ${:.2}", cost, self.max_cost_usd);
        }
        Ok(())
    }

    pub fn estimate_cost_usd(&self, model: &str) -> f64 {
        let (input_price, output_price) = match model {
            "gpt-4o" => (2.50, 10.00),
            "gpt-4o-mini" => (0.15, 0.60),
            "claude-3-5-sonnet" | "claude-3.5-sonnet" => (3.00, 15.00),
            "claude-3-haiku" => (0.25, 1.25),
            _ => (0.15, 0.60),
        };
        (self.total_input_tokens as f64 / 1_000_000.0 * input_price)
            + (self.total_output_tokens as f64 / 1_000_000.0 * output_price)
    }

    pub fn summary(&self) -> CostSummary {
        CostSummary {
            total_input_tokens: self.total_input_tokens,
            total_output_tokens: self.total_output_tokens,
            total_requests: self.total_requests,
            estimated_usd: self.estimate_cost_usd("gpt-4o-mini"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_requests: usize,
    pub estimated_usd: f64,
}
