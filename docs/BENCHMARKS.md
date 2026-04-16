# FoxNIO Benchmarks

FoxNIO 这一轮不再只喊“高性能”，而是把 **批量入口吞吐** 和 **号池调度热路径** 变成可复现基准。

## 目标基线

- 批量预检：看 `dry_run` 吞吐，确认控制面大批次输入不会先卡死在格式校验
- 批量导入：看 `fast-import` 的 `duration_ms` / `throughput_items_per_sec`
- 代理热路径：继续用现有 HTTP/2 / load tests 压请求吞吐
- 观测闭环：Prometheus 暴露批量操作耗时、最近吞吐、最近批次规模

## 新增批量观测

本轮已补以下指标：

- `foxnio_batch_operations_total`
- `foxnio_batch_operations_by_type_total{operation,mode}`
- `foxnio_batch_items_processed_total`
- `foxnio_batch_errors_total`
- `foxnio_batch_operation_duration_seconds{operation,mode}`
- `foxnio_batch_operation_throughput_items_per_second{operation,mode}`
- `foxnio_batch_operation_last_size{operation,mode}`

重点 operation：

- `fast_import_preview`
- `fast_import`
- `batch_create_accounts`
- `import_accounts_data`
- `batch_update_credentials`
- `batch_set_status`
- `batch_set_group`
- `batch_clear_rate_limit`
- `batch_refresh_tier`

## 快速跑批量 benchmark

准备一个管理员 JWT：

```bash
export FOXNIO_BASE_URL=http://localhost:8080
export FOXNIO_ADMIN_BEARER='<admin-jwt>'
```

### 1. 只测预检吞吐

```bash
python3 scripts/benchmark_fast_import.py --count 5000 --dry-run
```

### 2. 测可信数据源 fast mode

```bash
python3 scripts/benchmark_fast_import.py --count 5000 --fast-mode
```

### 3. 测标准校验导入

```bash
python3 scripts/benchmark_fast_import.py --count 2000
```

### 4. 测多 provider 混合号池

```bash
python3 scripts/benchmark_fast_import.py \
  --count 9000 \
  --providers openai,anthropic,gemini \
  --fast-mode \
  --repeat 3 \
  --format markdown
```

这条口径更贴近“大规模号池运营”真实场景：不是单一 provider 灌入，而是混合 provider 一次批量进池。

### 5. 输出逐次结果供运营侧留档

```bash
python3 scripts/benchmark_fast_import.py \
  --count 5000 \
  --providers openai,anthropic \
  --dry-run \
  --repeat 5 \
  --format json > /tmp/foxnio-fast-import-bench.json
```

脚本会打印：

- HTTP 状态
- 总条数
- 耗时 `duration_ms`
- 吞吐 `throughput_items_per_sec`
- 多次重复跑时的平均吞吐 / 平均耗时 / best run / worst run
- 请求侧 provider mix
- provider 维度导入汇总

## 查看指标

```bash
curl -s http://localhost:8080/metrics | grep foxnio_batch_
```

推荐重点盯：

- `foxnio_batch_operation_duration_seconds`
- `foxnio_batch_operation_throughput_items_per_second`
- `foxnio_batch_operation_last_size`

若同一轮 benchmark 有多次重复，建议把脚本输出与 `/metrics` 同时留档，便于区分：

- 接口返回耗时：服务端统计
- `wall_clock_duration_ms`：客户端真实观测
- Prometheus histogram：系统侧长期趋势

## 现有热路径基准

仓库里已有两类测试可继续复用：

- [`/home/telagod/project/foxnio/backend/tests/load_test.rs`](/home/telagod/project/foxnio/backend/tests/load_test.rs)
- [`/home/telagod/project/foxnio/backend/tests/http2_benchmark_test.rs`](/home/telagod/project/foxnio/backend/tests/http2_benchmark_test.rs)

建议口径：

1. **批量控制面**：`benchmark_fast_import.py`
2. **代理入口**：`load_test.rs`
3. **协议/连接层**：`http2_benchmark_test.rs`

## FoxNIO benchmark methodology（对外口径草案）

FoxNIO 后续对外讲性能时，统一按下面口径，避免“只贴一个好看的数字”：

### 1. 场景拆分

- **Control plane / batch**：导入、预检、批量更新、批量凭证轮换
- **Gateway hot path**：`/v1/chat/completions`、`/v1/responses`、`/v1/messages`
- **Connection layer**：HTTP/2、streaming、WebSocket / Realtime

### 2. 样本规模

- `1k accounts`：功能回归口径
- `10k accounts`：常规中型号池口径
- `100k accounts`：大规模号池目标口径

### 3. provider mix

- `single-provider`：只灌 `openai`
- `dual-provider`：`openai + anthropic`
- `mixed-provider`：`openai + anthropic + gemini`

### 4. 必报指标

- `duration_ms`
- `throughput_items_per_sec`
- `wall_clock_duration_ms`
- `error_rate`
- `provider_distribution`
- `skipped / failed reason topN`

### 5. 服务端同步留档

- `/metrics` 抓取 `foxnio_batch_*`
- admin stats 页截图或导出最近一次 ops snapshot
- benchmark 命令、样本规模、provider mix、repeat 次数一起记录

### 6. 对外表述规则

- 批量性能强调 **大批次导入吞吐** 与 **回显真实性**
- 网关性能强调 **附加开销、故障恢复、调度稳定性**
- 不直接拿 batch 吞吐去对比纯 proxy RPS；两类口径必须分开

## 建议留档格式

每轮 benchmark 至少留一份 markdown 记录，建议包含：

```text
date:
commit:
scenario:
accounts:
provider_mix:
repeat:
avg_duration_ms:
avg_throughput:
best_run:
worst_run:
server_metrics_snapshot:
notes:
```

## 下一步

下一轮应继续补：

1. scheduler cache / sticky session / cooldown 的 Prometheus 指标
2. 大号池样本数据生成器（1k / 10k / 100k 账号）
3. 基准结果固化到 `docs/EVOLUTION_TRACK_2026-04.md`
4. 为 `load_test.rs` / `http2_benchmark_test.rs` 补固定环境说明（CPU、内存、上游延迟模拟、连接数）
5. 建 `docs/benchmarks/` 目录，按日期沉淀结果
