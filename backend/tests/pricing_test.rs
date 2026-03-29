#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 模型定价测试

#[cfg(test)]
mod tests {

    #[test]
    fn test_pricing_gpt4_turbo() {
        // GPT-4 Turbo: $0.01/1K input, $0.03/1K output
        let input_rate = 1000; // 分/1K tokens = $0.01
        let output_rate = 3000; // 分/1K tokens = $0.03

        let input_tokens = 1000;
        let output_tokens = 500;

        let input_cost = (input_tokens as f64 / 1000.0 * input_rate as f64) as i64;
        let output_cost = (output_tokens as f64 / 1000.0 * output_rate as f64) as i64;
        let total_cost = input_cost + output_cost;

        assert_eq!(input_cost, 1000);
        assert_eq!(output_cost, 1500);
        assert_eq!(total_cost, 2500); // 25 分 = $0.25
    }

    #[test]
    fn test_pricing_claude_opus() {
        // Claude 3 Opus: $0.015/1K input, $0.075/1K output
        let input_rate = 1500;
        let output_rate = 7500;

        let input_tokens = 2000;
        let output_tokens = 1000;

        let input_cost = (input_tokens as f64 / 1000.0 * input_rate as f64) as i64;
        let output_cost = (output_tokens as f64 / 1000.0 * output_rate as f64) as i64;
        let total_cost = input_cost + output_cost;

        assert_eq!(input_cost, 3000);
        assert_eq!(output_cost, 7500);
        assert_eq!(total_cost, 10500); // 105 分 = $1.05
    }

    #[test]
    fn test_pricing_gemini_flash() {
        // Gemini 1.5 Flash: $0.00035/1K input, $0.00105/1K output
        let input_rate = 35;
        let output_rate = 105;

        let input_tokens = 10000;
        let output_tokens = 5000;

        let input_cost = (input_tokens as f64 / 1000.0 * input_rate as f64) as i64;
        let output_cost = (output_tokens as f64 / 1000.0 * output_rate as f64) as i64;
        let total_cost = input_cost + output_cost;

        assert_eq!(input_cost, 350);
        assert_eq!(output_cost, 525);
        assert_eq!(total_cost, 875); // 8.75 分 = $0.0875
    }

    #[test]
    fn test_pricing_deepseek() {
        // DeepSeek Chat: $0.001/1K input, $0.002/1K output
        let input_rate = 10;
        let output_rate = 20;

        let input_tokens = 10000;
        let output_tokens = 5000;

        let input_cost = (input_tokens as f64 / 1000.0 * input_rate as f64) as i64;
        let output_cost = (output_tokens as f64 / 1000.0 * output_rate as f64) as i64;
        let total_cost = input_cost + output_cost;

        assert_eq!(input_cost, 100);
        assert_eq!(output_cost, 100);
        assert_eq!(total_cost, 200); // 2 分 = $0.02
    }

    #[test]
    fn test_pricing_rate_multiplier() {
        let base_cost = 100;
        let multiplier = 1.5;

        let final_cost = (base_cost as f64 * multiplier) as i64;

        assert_eq!(final_cost, 150);
    }

    #[test]
    fn test_model_price_comparison() {
        // 比较不同模型的成本

        let tokens = (1000, 500); // input, output

        let models: Vec<(&str, f64, f64)> = vec![
            ("gpt-4-turbo", 10.0, 30.0),
            ("gpt-4o", 2.5, 10.0),
            ("gpt-4o-mini", 0.15, 0.6),
            ("claude-3-opus", 15.0, 75.0),
            ("claude-3-sonnet", 3.0, 15.0),
            ("claude-3-haiku", 0.25, 1.25),
        ];

        let mut costs = Vec::new();

        for (model, input_rate, output_rate) in &models {
            let cost = (tokens.0 as f64 / 1000.0 * input_rate
                + tokens.1 as f64 / 1000.0 * output_rate) as i64;
            costs.push((*model, cost));
        }

        // Claude 3 Haiku 应该是最便宜的
        let haiku_cost = costs
            .iter()
            .find(|(m, _)| m == &"claude-3-haiku")
            .map(|(_, c)| *c)
            .unwrap();

        let opus_cost = costs
            .iter()
            .find(|(m, _)| m == &"claude-3-opus")
            .map(|(_, c)| *c)
            .unwrap();

        assert!(haiku_cost < opus_cost);
    }
}
