//! Tests for scheduled test plan service

#[cfg(test)]
mod tests {
    use foxnio::entity::scheduled_test_results::TestResultStatus;

    #[test]
    fn test_result_status() {
        assert_eq!(TestResultStatus::Success.as_str(), "success");
        assert_eq!(TestResultStatus::Failed.as_str(), "failed");
        assert_eq!(TestResultStatus::Timeout.as_str(), "timeout");

        assert_eq!(
            TestResultStatus::parse("success"),
            TestResultStatus::Success
        );
        assert_eq!(TestResultStatus::parse("failed"), TestResultStatus::Failed);
        assert_eq!(
            TestResultStatus::parse("timeout"),
            TestResultStatus::Timeout
        );
        assert_eq!(TestResultStatus::parse("unknown"), TestResultStatus::Failed);
    }
}
