# Python error handling

[简体中文](errors_cn.md) · [Examples](examples.md) · [Configuration](configuration.md)

Method calls raise exceptions on error. If a log is accepted but delivery fails, `on_delivery` receives `DeliveryError`.
Flush and close do not report individual delivery failures.

## Exceptions from method calls

| Exception | Meaning and action |
| --- | --- |
| `EnqueueFullError` | The queue is full and the log was not accepted. Retry later. |
| `ProducerClosedError` | Producer is closing or closed. Stop sending or create a new Producer. |
| `ProducerError` | Producer could not start, encountered an error, or was used incorrectly. Check the error message. |
| `ValueError` | Invalid configuration, log time, destination name, or other argument. Check the error message. |
| `TypeError` | An argument has the wrong type. Check the error message and supplied value. |
| `OverflowError` | A number is outside the supported range. Check the supplied value. |

Use `except ProducerError` to catch both `EnqueueFullError` and `ProducerClosedError`.
These two send errors do not trigger a callback. See the [usage examples](examples.md) for a retry example.

## Delivery errors in callbacks

`on_delivery` receives `None` on success or `DeliveryError` on failure. `DeliveryError` is not raised as an exception; handle it in the callback.

| `error.kind` | Meaning and action |
| --- | --- |
| `server` | SLS returned an error. Use the error code to check permissions, the target Logstore, or other settings. |
| `network` | The network request failed. Check connectivity and the SLS endpoint. |
| `credentials` | Could not obtain credentials. Check whether the credential source is available. |
| `timeout` | Delivery did not finish within `delivery_timeout`. Check the network and service, and adjust the timeout if needed. |
| `invalid_response` | The service response could not be read. Check the endpoint and proxy settings. |
| `internal` | Log processing failed. Keep the error message for troubleshooting. |
| `unknown` | An unclassified delivery error. See `message` for details. |

`message` describes the error. For SLS errors, inspect `error_code`, `http_status`, and `request_id` for more details.
Keep the error message and request ID for troubleshooting. Authentication and permission failures are `server` errors.
