# 测试修复计划

## 失败测试清单 (31个)

### 1. 实体测试 (1个)
- test_api_key_masking

### 2. 网关测试 (12个)
- test_validator_system_prompt
- test_has_claude_code_system_prompt
- test_compress_brotli
- test_compress_gzip
- test_compression_ratio
- test_decompress_brotli
- test_decompress_gzip
- test_select_encoding_with_accept_header
- test_stats
- test_check_permission_admin
- test_check_permission_user
- test_latency_optimized_selection
- test_cost_score

### 3. 指标测试 (3个)
- test_business_metrics_account_usage
- test_business_metrics_summary
- test_global_business_metrics

### 4. 服务测试 (12个)
- test_calculate_cost_* (7个 - billing未实现)
- test_generate_setup_token_url (OAuth)
- test_dynamic_permission_update (runtime问题)
- test_has_permission (runtime问题)
- test_role_permissions (runtime问题)
- test_quota_tracking

### 5. 工具测试 (2个)
- test_generate_api_key
- test_mask_string

## 修复策略

### 高优先级 (立即修复)
1. 服务测试 - 实现缺失功能
2. 工具测试 - 简单断言修复

### 中优先级
3. 实体测试
4. 网关测试

### 低优先级
5. 指标测试 - 可能需要mock数据

