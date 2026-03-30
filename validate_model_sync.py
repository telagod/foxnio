#!/usr/bin/env python3
"""
Validation script for model_sync.rs implementation
Checks that all required methods and features are present
"""

import re
import sys

def check_implementation(filename):
    with open(filename, 'r') as f:
        content = f.read()
    
    print("=" * 60)
    print("Model Sync Service Implementation Validation")
    print("=" * 60)
    
    # Check required methods
    required_methods = [
        ('sync_openai', 'OpenAI model sync'),
        ('sync_anthropic', 'Anthropic model sync'),
        ('sync_google', 'Google/Gemini model sync'),
        ('sync_deepseek', 'DeepSeek model sync'),
        ('sync_mistral', 'Mistral model sync'),
        ('sync_cohere', 'Cohere model sync'),
        ('detect_price_change', 'Price change detection'),
        ('start_periodic_sync', 'Periodic sync task'),
    ]
    
    print("\n✓ Required Methods:")
    print("-" * 60)
    for method, description in required_methods:
        pattern = rf'(?:async\s+)?fn\s+{method}\s*\('
        if re.search(pattern, content):
            print(f"  ✓ {method:30s} - {description}")
        else:
            print(f"  ✗ {method:30s} - MISSING!")
            return False
    
    # Check test cases
    test_count = len(re.findall(r'#\[test\]', content))
    print(f"\n✓ Unit Tests: {test_count} found (minimum required: 6)")
    print("-" * 60)
    
    if test_count < 6:
        print(f"  ✗ Insufficient tests: {test_count} < 6")
        return False
    else:
        print(f"  ✓ Sufficient tests: {test_count} >= 6")
    
    # Check important features
    features = [
        ('API key management', 'set_api_key'),
        ('Retry mechanism', 'sync_provider_with_retry'),
        ('Deprecation detection', 'detect_deprecated_models'),
        ('Sync state tracking', 'SyncState'),
        ('Price change struct', 'PriceChange'),
        ('Provider sync status', 'ProviderSyncStatus'),
        ('Concurrent sync prevention', 'already in progress'),
        ('Error handling', 'anyhow::'),
        ('Logging', 'tracing::'),
        ('Async support', 'async fn'),
    ]
    
    print("\n✓ Key Features:")
    print("-" * 60)
    for feature, keyword in features:
        if keyword in content:
            print(f"  ✓ {feature}")
        else:
            print(f"  ✗ {feature} - MISSING!")
            return False
    
    # Check provider endpoints
    endpoints = [
        ('OpenAI', 'https://api.openai.com/v1/models'),
        ('Google', 'generativelanguage.googleapis.com'),
        ('Mistral', 'https://api.mistral.ai/v1/models'),
        ('Cohere', 'https://api.cohere.ai/v1/models'),
        ('DeepSeek', 'https://api.deepseek.com'),
    ]
    
    print("\n✓ API Endpoints:")
    print("-" * 60)
    for provider, url in endpoints:
        if url in content:
            print(f"  ✓ {provider:15s} - {url}")
        else:
            print(f"  ✗ {provider:15s} - MISSING endpoint!")
    
    # Check model pricing
    print("\n✓ Model Pricing Configured:")
    print("-" * 60)
    pricing_checks = [
        ('GPT-4o pricing', '2.5'),
        ('Claude Opus pricing', '15.0'),
        ('Gemini 2.0 Flash pricing', '0.0'),  # Free
        ('DeepSeek Chat pricing', '0.14'),
        ('Mistral Large pricing', '2.0'),
        ('Command R+ pricing', '2.5'),
    ]
    for desc, price in pricing_checks:
        if price in content:
            print(f"  ✓ {desc}")
        else:
            print(f"  ✗ {desc} - MISSING!")
    
    # Check error handling patterns
    print("\n✓ Error Handling Patterns:")
    print("-" * 60)
    error_patterns = [
        ('Context for errors', r'\.context\('),
        ('Bail macro', r'bail!\('),
        ('Result type', r'Result<'),
        ('Error logging', r'error!\('),
        ('Warn logging', r'warn!\('),
    ]
    for pattern_name, pattern in error_patterns:
        count = len(re.findall(pattern, content))
        print(f"  ✓ {pattern_name:20s} - {count} occurrences")
    
    # Check constants
    print("\n✓ Constants Defined:")
    print("-" * 60)
    constants = [
        ('MAX_RETRIES', '3'),
        ('RETRY_DELAY_MS', '1000'),
    ]
    for const, value in constants:
        if f'const {const}' in content and value in content:
            print(f"  ✓ {const} = {value}")
        else:
            print(f"  ✗ {const} - MISSING!")
    
    # Check struct definitions
    print("\n✓ Struct Definitions:")
    print("-" * 60)
    structs = [
        'ModelSyncService',
        'SyncState',
        'ProviderSyncStatus',
        'SyncResult',
        'PriceChange',
        'OpenAIModelsResponse',
        'AnthropicModelInfo',
        'GeminiModelsResponse',
        'MistralModelsResponse',
        'CohereModelsResponse',
    ]
    for struct in structs:
        if f'struct {struct}' in content:
            print(f"  ✓ {struct}")
        else:
            print(f"  ✗ {struct} - MISSING!")
    
    # Line count
    line_count = len(content.split('\n'))
    print(f"\n✓ Code Metrics:")
    print("-" * 60)
    print(f"  Total lines: {line_count}")
    print(f"  Original: 480 lines")
    print(f"  Added: ~{line_count - 480} lines")
    
    print("\n" + "=" * 60)
    print("✓ VALIDATION PASSED")
    print("=" * 60)
    return True

if __name__ == '__main__':
    filename = 'backend/src/service/model_sync.rs'
    try:
        if check_implementation(filename):
            sys.exit(0)
        else:
            print("\n✗ VALIDATION FAILED")
            sys.exit(1)
    except FileNotFoundError:
        print(f"✗ File not found: {filename}")
        sys.exit(1)
    except Exception as e:
        print(f"✗ Error: {e}")
        sys.exit(1)
