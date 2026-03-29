//! 计费服务测试

#[cfg(test)]
#[allow(clippy::all)]
mod tests {
    use crate::service::billing::{BillingService, UserStats};

    #[test]
    fn test_calculate_cost_gpt4_turbo() {
        // GPT-4 Turbo: 1000 cents/1K input, 3000 cents/1K output
        // 1000 input + 500 output = 1000 + 1500 = 2500 cents
        let cost = BillingService::calculate_cost_static("gpt-4-turbo", 1000, 500, 1.0);
        assert_eq!(cost, 2500);
    }

    #[test]
    fn test_calculate_cost_claude_opus() {
        // Claude 3 Opus: 1500 cents/1K input, 7500 cents/1K output
        // 1000 input + 500 output = 1500 + 3750 = 5250 cents
        let cost = BillingService::calculate_cost_static("claude-3-opus-20240229", 1000, 500, 1.0);
        assert_eq!(cost, 5250);
    }

    #[test]
    fn test_calculate_cost_gemini_flash() {
        // Gemini 1.5 Flash: 35 cents/1K input, 105 cents/1K output
        // 1000 input + 500 output = 35 + 52 = 87 cents
        let cost = BillingService::calculate_cost_static("gemini-1.5-flash", 1000, 500, 1.0);
        assert_eq!(cost, 87);
    }

    #[test]
    fn test_calculate_cost_gemini_flash_large() {
        // Larger request: 100K input + 50K output
        // 100000 input + 50000 output = 3500 + 5250 = 8750 cents
        let cost = BillingService::calculate_cost_static("gemini-1.5-flash", 100_000, 50_000, 1.0);
        assert_eq!(cost, 8750);
    }

    #[test]
    fn test_calculate_cost_with_multiplier() {
        // GPT-4 Turbo with 1.5x multiplier: 2500 * 1.5 = 3750 cents
        let cost = BillingService::calculate_cost_static("gpt-4-turbo", 1000, 500, 1.5);
        assert_eq!(cost, 3750);
    }

    #[test]
    fn test_calculate_cost_unknown_model() {
        // Unknown model uses default rate: 100 cents/1K input, 300 cents/1K output
        // 1000 input + 500 output = 100 + 150 = 250 cents
        let cost = BillingService::calculate_cost_static("unknown-model", 1000, 500, 1.0);
        assert_eq!(cost, 250);
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        let cost = BillingService::calculate_cost_static("gpt-4-turbo", 0, 0, 1.0);
        assert_eq!(cost, 0);
    }

    #[test]
    fn test_user_stats_default() {
        let stats = UserStats {
            total_requests: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost: 0,
        };

        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.total_cost, 0);
    }
}
