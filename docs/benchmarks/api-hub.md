# API Hub 基准压测

可复现的 API Hub 并发吞吐基线，用于回归对比（提交 ff41482 引入）。

## 运行方式

```bash
pnpm bench                    # 默认：50 并发 × 200 请求
BENCH_CONCURRENCY=100 BENCH_TOTAL=500 pnpm bench   # 自定义并发与总量
```

## 基线（2026-08-01，本机 16 核 Linux，debug 构建）

| 指标 | 值 |
|---|---|
| 并发 | 50 |
| 总请求 | 200 |
| 失败 | 0 |
| RPS | 3571 |
| p50 延迟 | 6.4 ms |
| p95 延迟 | 36.2 ms |

## 说明

- 压测走完整链路：前端请求 → API Hub（axum）→ mock 上游，覆盖协议转换与 usage 日志回填
- 使用 **debug 构建**（`cargo run` 默认 profile）；发布构建（LTO + strip）吞吐会更高
- 若 RPS 或 p95 出现数量级退化（如 RPS < 2000 或 p95 > 100ms），应检查 API Hub 是否引入新的同步阻塞或锁竞争
