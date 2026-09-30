# Producer metrics

To collect metrics, set up a `metrics::Recorder` before creating the Producer.
Use the exporter of your choice to send them to your monitoring system. Without a Recorder, no metrics are collected.

Counters combine all Producers in the process. They do not separate projects or Logstores.

| Counter                                   | What it tells you                                                                                        |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `sls_producer_accepted_logs_total`        | Logs accepted by the Producer, including those that later fail to send.                                  |
| `sls_producer_delivered_logs_total`       | Logs that succeeded (`result=success`) or failed (`result=failed`). Retried logs count once.             |
| `sls_producer_delivered_raw_bytes_total`  | Estimated size in bytes of successfully delivered logs, before compression. Retried logs count once.     |
| `sls_producer_rejected_submissions_total` | Sends rejected because the Producer is full or closed. These logs are not included in delivery failures. |

Use the byte count to estimate log volume, not network traffic.

Metrics may have a short delay.
