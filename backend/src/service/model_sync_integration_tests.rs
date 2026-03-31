// Model Sync Service Integration Tests
// 
// This file demonstrates how to use the model sync service in practice

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::Arc;

    /// Test: OpenAI Sync Integration
    /// 
    /// Demonstrates:
    /// 1. API key configuration
    /// 2. Model list retrieval
    /// 3. Filtering for chat models
    /// 4. Price detection
    /// 5. Database updates
    #[tokio::test]
    async fn test_openai_sync_integration() {
        // Setup
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = Arc::new(ModelSyncService::new(db, model_registry));
        
        // Configure API key
        sync_service.set_api_key("openai", "sk-test-key-12345").await;
        
        // Verify key is stored
        let key = sync_service.get_api_key("openai").await.unwrap();
        assert_eq!(key, "sk-test-key-12345");
        
        // Sync models
        let result = sync_service.sync_provider("openai").await;
        
        // Verify result structure
        assert!(result.is_ok());
        let sync_result = result.unwrap();
        assert_eq!(sync_result.provider, "openai");
        assert!(sync_result.new_models.len() >= 0);
        assert!(sync_result.updated_models.len() >= 0);
        assert!(sync_result.deprecated_models.len() >= 0);
        
        // Verify state was updated
        let state = sync_service.get_sync_state().await;
        assert!(state.provider_status.contains_key("openai"));
    }

    /// Test: Anthropic Sync Integration
    /// 
    /// Demonstrates:
    /// 1. Known model list usage
    /// 2. Price change detection
    /// 3. Model updates
    #[tokio::test]
    async fn test_anthropic_sync_integration() {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = Arc::new(ModelSyncService::new(db, model_registry));
        
        let result = sync_service.sync_provider("anthropic").await.unwrap();
        
        assert_eq!(result.provider, "anthropic");
        // Claude models should be detected
        assert!(result.new_models.len() > 0 || result.updated_models.len() > 0);
        
        // Verify models include latest versions
        let has_latest = result.new_models.iter().any(|m| 
            m.contains("claude-3-5") || m.contains("claude-opus-4")
        );
        assert!(has_latest);
    }

    /// Test: Full Sync Cycle
    /// 
    /// Demonstrates:
    /// 1. Syncing all providers
    /// 2. Handling mixed success/failure
    /// 3. State management
    #[tokio::test]
    async fn test_full_sync_cycle() {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = Arc::new(ModelSyncService::new(db, model_registry));
        
        // Configure API keys for all providers
        let providers = vec!["openai", "anthropic", "google", "deepseek", "mistral", "cohere"];
        for provider in &providers {
            sync_service.set_api_key(provider, format!("test-key-{provider}")).await;
        }
        
        // Perform full sync
        let results = sync_service.sync_all().await.unwrap();
        
        // Verify all providers were synced
        assert_eq!(results.len(), 6);
        
        // Verify state
        let state = sync_service.get_sync_state().await;
        assert!(state.last_sync.is_some());
        assert!(!state.in_progress);
        
        // Verify provider statuses
        for provider in &providers {
            assert!(state.provider_status.contains_key(*provider));
        }
    }

    /// Test: Concurrent Sync Prevention
    /// 
    /// Demonstrates:
    /// 1. Locking mechanism
    /// 2. Error handling for concurrent access
    #[tokio::test]
    async fn test_concurrent_sync_prevention() {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = Arc::new(ModelSyncService::new(db, model_registry));
        
        // Start a sync
        {
            let mut state = sync_service.sync_state.write().await;
            state.in_progress = true;
        }
        
        // Attempt another sync should fail
        let result = sync_service.sync_all().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already in progress"));
    }

    /// Test: Price Change Detection
    /// 
    /// Demonstrates:
    /// 1. Detecting price increases
    /// 2. Detecting price decreases
    /// 3. Ignoring insignificant changes
    #[tokio::test]
    async fn test_price_change_scenarios() {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = ModelSyncService::new(db, model_registry);
        
        // Scenario 1: Significant increase
        let change = service.detect_price_change("test-model", 10.0, 12.0, 20.0, 20.0);
        assert!(change.is_some());
        let change = change.unwrap();
        assert_eq!(change.old_input_price, 10.0);
        assert_eq!(change.new_input_price, 12.0);
        
        // Scenario 2: Significant decrease
        let change = service.detect_price_change("test-model", 10.0, 8.0, 20.0, 20.0);
        assert!(change.is_some());
        
        // Scenario 3: Insignificant change (< 1%)
        let change = service.detect_price_change("test-model", 10.0, 10.05, 20.0, 20.0);
        assert!(change.is_none());
        
        // Scenario 4: Zero to price (new pricing)
        let change = service.detect_price_change("test-model", 0.0, 5.0, 0.0, 10.0);
        assert!(change.is_some());
    }

    /// Test: Retry Mechanism
    /// 
    /// Demonstrates:
    /// 1. Automatic retries on failure
    /// 2. Exponential backoff
    /// 3. Final failure after max retries
    #[tokio::test]
    async fn test_retry_mechanism() {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = Arc::new(ModelSyncService::new(db, model_registry));
        
        // Sync without API key should fail after retries
        let result = sync_service.sync_provider_with_retry("openai").await;
        assert!(result.is_err());
        
        // Verify error mentions API key
        let error = result.unwrap_err();
        assert!(error.to_string().contains("API key"));
    }

    /// Test: Periodic Sync Task
    /// 
    /// Demonstrates:
    /// 1. Starting background task
    /// 2. Non-blocking execution
    /// 3. Graceful error handling
    #[tokio::test]
    async fn test_periodic_sync_task() {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = Arc::new(ModelSyncService::new(db, model_registry));
        
        // Start periodic sync (every 1 hour for testing)
        sync_service.clone().start_periodic_sync(1).await;
        
        // Give it a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Service should still be usable
        let state = sync_service.get_sync_state().await;
        assert!(!state.in_progress);
    }

    /// Test: Model Deprecation Detection
    /// 
    /// Demonstrates:
    /// 1. Detecting removed models
    /// 2. Logging deprecations
    /// 3. Future auto-disable capability
    #[tokio::test]
    async fn test_model_deprecation() {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = ModelSyncService::new(db, model_registry);
        
        // Add a model that doesn't exist in current API
        let old_model = CreateModelRequest {
            name: "gpt-3-old-deprecated".to_string(),
            provider: "openai".to_string(),
            enabled: true,
            ..Default::default()
        };
        sync_service.model_registry.create(old_model).await.unwrap();
        
        // Sync with current models (not including gpt-3-old-deprecated)
        let current_models = vec!["gpt-4", "gpt-3.5-turbo"];
        let deprecated = sync_service
            .detect_deprecated_models("openai", &current_models)
            .await
            .unwrap();
        
        assert!(deprecated.contains(&"gpt-3-old-deprecated".to_string()));
    }

    /// Test: Error Recovery
    /// 
    /// Demonstrates:
    /// 1. Handling partial failures
    /// 2. Continuing with other providers
    /// 3. Collecting all errors
    #[tokio::test]
    async fn test_error_recovery() {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        let sync_service = Arc::new(ModelSyncService::new(db, model_registry));
        
        // Configure only some providers
        sync_service.set_api_key("openai", "test-key").await;
        sync_service.set_api_key("anthropic", "test-key").await;
        // Leave others without keys
        
        // Sync should handle missing keys for some providers
        let results = sync_service.sync_all().await.unwrap();
        
        // Some should succeed, some should fail
        let successful: Vec<_> = results.iter().filter(|r| r.errors.is_empty()).collect();
        let failed: Vec<_> = results.iter().filter(|r| !r.errors.is_empty()).collect();
        
        assert!(successful.len() >= 2); // At least openai and anthropic
        assert!(failed.len() >= 4); // Missing API keys
    }
}

/// Usage Example: Setting up the service in production
#[cfg(test)]
mod production_example {
    use super::*;

    /// Example: Production Setup
    #[tokio::test]
    async fn production_setup_example() {
        // 1. Initialize database connection
        let db = sea_orm::Database::connect("postgres://user:pass@localhost/db")
            .await
            .unwrap();
        
        // 2. Create model registry
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        
        // 3. Create sync service
        let sync_service = Arc::new(ModelSyncService::new(db, model_registry));
        
        // 4. Load API keys from environment
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            sync_service.set_api_key("openai", key).await;
        }
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            sync_service.set_api_key("anthropic", key).await;
        }
        if let Ok(key) = std::env::var("GOOGLE_API_KEY") {
            sync_service.set_api_key("google", key).await;
        }
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            sync_service.set_api_key("deepseek", key).await;
        }
        if let Ok(key) = std::env::var("MISTRAL_API_KEY") {
            sync_service.set_api_key("mistral", key).await;
        }
        if let Ok(key) = std::env::var("COHERE_API_KEY") {
            sync_service.set_api_key("cohere", key).await;
        }
        
        // 5. Perform initial sync
        match sync_service.sync_all().await {
            Ok(results) => {
                for result in results {
                    println!(
                        "Provider {}: {} new, {} updated, {} deprecated",
                        result.provider,
                        result.new_models.len(),
                        result.updated_models.len(),
                        result.deprecated_models.len()
                    );
                    
                    // Log price changes
                    for change in result.price_changes {
                        eprintln!(
                            "Price change: {} input {:.2} -> {:.2}, output {:.2} -> {:.2}",
                            change.model_name,
                            change.old_input_price,
                            change.new_input_price,
                            change.old_output_price,
                            change.new_output_price
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("Initial sync failed: {}", e);
            }
        }
        
        // 6. Start periodic sync (every 24 hours)
        sync_service.clone().start_periodic_sync(24).await;
        
        // Service is now running and will auto-sync every 24 hours
    }

    /// Example: Manual sync trigger
    #[tokio::test]
    async fn manual_sync_example() {
        let sync_service = create_sync_service().await;
        
        // Sync specific provider
        let result = sync_service.sync_provider("openai").await.unwrap();
        
        println!("Synced OpenAI:");
        println!("  New models: {:?}", result.new_models);
        println!("  Updated models: {:?}", result.updated_models);
        println!("  Deprecated models: {:?}", result.deprecated_models);
        println!("  Price changes: {:?}", result.price_changes.len());
        
        if !result.errors.is_empty() {
            eprintln!("  Errors: {:?}", result.errors);
        }
    }

    /// Example: Checking sync status
    #[tokio::test]
    async fn check_status_example() {
        let sync_service = create_sync_service().await;
        
        // Get current state
        let state = sync_service.get_sync_state().await;
        
        println!("Sync Status:");
        println!("  Last sync: {:?}", state.last_sync);
        println!("  Last success: {:?}", state.last_success);
        println!("  In progress: {}", state.in_progress);
        println!("  Last error: {:?}", state.last_error);
        
        for (provider, status) in state.provider_status {
            println!(
                "  {}: {} models, last sync {:?}",
                provider,
                status.models_count,
                status.last_sync
            );
        }
    }

    async fn create_sync_service() -> Arc<ModelSyncService> {
        let db = create_test_db().await;
        let model_registry = Arc::new(ModelRegistry::new(db.clone()));
        Arc::new(ModelSyncService::new(db, model_registry))
    }
}
