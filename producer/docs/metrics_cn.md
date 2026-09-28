# Producer 统计指标

[English](metrics.md) · [概览](../README_CN.md)

需要统计发送情况时，在创建 Producer 前设置 `metrics::Recorder`，并通过对应的 exporter 接入监控系统。
未设置 Recorder 时，不收集指标。

统计会合并进程中所有 Producer 的数据，不区分 Project 和 Logstore。

| 指标 | 用途 |
| --- | --- |
| `sls_producer_accepted_logs_total` | Producer 已接收的日志条数，包括后续发送失败的日志。 |
| `sls_producer_delivered_logs_total` | 发送成功（`result=success`）或失败（`result=failed`）的日志条数。重试不会重复计数。 |
| `sls_producer_delivered_raw_bytes_total` | 成功发送日志的预估原始字节数，未压缩。重试不会重复计数。 |
| `sls_producer_rejected_submissions_total` | 因队列已满或 Producer 已关闭而被拒绝的发送次数，不计入发送失败条数。 |

原始字节数按所有 key、value 的 UTF-8 大小加上每条日志 16 字节估算，适合了解日志量，不代表网络流量。

发送统计通常每秒更新一次，关闭完成前会再更新一次。负载较高时，更新可能稍有延迟。
