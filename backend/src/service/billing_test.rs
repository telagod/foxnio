//! 计费服务测试

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> BillingService {
        BillingService {
            db: todo!(), // 需要 mock
            rate_multiplier: 1.0,
        }
    }

    #[test]
    fn test_calculate_cost_gpt4_turbo() {
        let service = create_test_service();
        
        // GPT-4 Turbo: $0.01/1K input, $0.03/1K output
        // 1000 input + 500 output = 10 + 15 = 25 分
        let cost = service.calculate_cost("gpt-4-turbo", 1000, 500);
        assert_eq!(cost, 25);
    }

    #[test]
    fn test_calculate_cost_claude_opus() {
        let service = create_test_service();
        
        // Claude 3 Opus: $0.015/1K input, $0.075/1K output
        // 1000 input + 500 output = 15 + 37.5 = 52 分 (四舍五入)
        let cost = service.calculate_cost("claude-3-opus-20240229", 1000, 500);
        assert_eq!(cost, 52);
    }

    #[test]
    fn test_calculate_cost_gemini_flash() {
        let service = create_test_service();
        
        // Gemini 1.5 Flash: $0.00035/1K input, $0.00105/1K output
        // 1000 input + 500 output = 0.35 + 0.525 = 0 分 (太小)
        let cost = service.calculate_cost("gemini-1.5-flash", 1000, 500);
        assert_eq!(cost, 0); // 舍入到 0
    }

    #[test]
    fn test_calculate_cost_gemini_flash_large() {
        let service = create_test_service();
        
        // 更大的请求
        // 100K input + 50K output = 35 + 52.5 = 87 分
        let cost = service.calculate_cost("gemini-1.5-flash", 100_000, 50_000);
        assert_eq!(cost, 87);
    }

    #[test]
    fn test_calculate_cost_with_multiplier() {
        let service = BillingService {
            db: todo!(),
            rate_multiplier: 1.5, // 1.5 倍费率
        };
        
        // GPT-4 Turbo with 1.5x: 25 * 1.5 = 37 分
        let cost = service.calculate_cost("gpt-4-turbo", 1000, 500);
        assert_eq!(cost, 37);
    }

    #[test]
    fn test_calculate_cost_unknown_model() {
        let service = create_test_service();
        
        // 未知模型使用默认费率
        // 默认: $0.001/1K input, $0.003/1K output
        let cost = service.calculate_cost("unknown-model", 1000, 500);
        assert_eq!(cost, 2); // 1 + 1.5 = 2.5 -> 2
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        let service = create_test_service();
        
        let cost = service.calculate_cost("gpt-4-turbo", 0, 0);
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
