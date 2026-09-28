# Producer metrics

Install a [`metrics::Recorder`](https://docs.rs/metrics/latest/metrics/trait.Recorder.html)
before `Producer::create(config)`. The producer registers counter handles once
and reuses them across workers. Handles created without a Recorder remain no-op.
The application owns export; no producer metrics configuration or polling API is needed.

All instances contribute to the same process-wide counters. There are no destination,
rejection-reason, latency or queue-depth metrics.

| Counter | Labels | Meaning |
| --- | --- | --- |
| `sls_producer_accepted_logs_total` | none | Logs admitted locally, including logs that later fail delivery |
| `sls_producer_delivered_logs_total` | `result=success` or `result=failed` | Logs with a terminal delivery outcome; each log is counted once, regardless of retries |
| `sls_producer_delivered_raw_bytes_total` | none | Estimated original bytes in successfully delivered logs; each log is counted once, regardless of retries |
| `sls_producer_rejected_submissions_total` | none | Send calls rejected because the producer is closed or admission capacity is unavailable |

The raw-byte counter uses the bytes unit; all other counters use the count unit.
It reuses the existing admission size estimate: UTF-8 key/value bytes plus 16 bytes
per log. It excludes group metadata, compression and HTTP overhead, and requires no
additional size calculation. Failed and rejected logs do not add to this byte count.

Rejected submissions are not accepted logs and do not contribute to delivery failures. Invalid destination names fail at writer creation
and do not count as rejected submissions. Callback panics are logged and do not change
the delivery outcome.

The existing IO runtime samples admission and delivery totals once per second and
once before close completes, including failure cleanup. Values can lag when the runtime
is busy. Flush does not force a sample. Sampling continues while close waits for
callbacks; even producers that close within one interval publish their final counts.
Rejections are recorded immediately, including sends attempted after close.
Internal lifecycle and capacity accounting is independent of these metrics.
