# Rust error handling

Method calls return `ProducerError`. If a log is accepted but delivery fails, the callback receives `DeliveryError`.
Flush and close do not report individual delivery failures.

## Errors from method calls

| `ProducerError` | Meaning and action                                                                         |
| --------------- | ------------------------------------------------------------------------------------------ |
| `Config`        | Invalid configuration. Check the setting named in the error message.                       |
| `Creation`      | Producer could not start. Check the error message and runtime environment.                 |
| `EnqueueFull`   | The queue is full and the log was not accepted. Retry later.                               |
| `Closed`        | Producer is closing or closed. Stop sending or create a new Producer.                      |
| `InvalidInput`  | Invalid Project or Logstore name. Check the names you supplied.                            |
| `Reentrant`     | Waiting for the same Producer from its callback. Call flush or close outside the callback. |
| `Internal`      | Producer encountered an error. Keep the error message for troubleshooting.                 |

`EnqueueFull` and `Closed` do not trigger a callback. Use `into_log()` to retrieve the unsent log and retry later or save it elsewhere.
See the [usage examples](examples.md) for a retry example.

## Delivery errors in callbacks

| `DeliveryError`   | Meaning and action                                                                                                                                          |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Server`          | SLS returned an error. Use the error code to check permissions, the target Logstore, or other settings.                                                     |
| `Network`         | The network request failed. Check connectivity and the SLS endpoint.                                                                                        |
| `Credentials`     | Could not obtain credentials. Check whether the credential source is available.                                                                             |
| `Timeout`         | The soft `delivery_timeout` budget was exhausted before processing or starting an attempt. Check the network and service, and adjust the timeout if needed. |
| `InvalidResponse` | The service response could not be read. Check the endpoint and proxy settings.                                                                              |
| `Internal`        | Log processing failed. Keep the error message for troubleshooting.                                                                                          |

Use `error_code()`, `http_status()`, and `request_id()` to inspect an SLS error.
Keep the error message and request ID for troubleshooting. Authentication and permission failures are `Server` errors.

## Credential errors

When creating credentials or writing a dynamic credential provider, you may encounter `CredentialsError`:

| Error              | Meaning                                                          |
| ------------------ | ---------------------------------------------------------------- |
| `InvalidAccessKey` | The AccessKey ID or secret is empty.                             |
| `Expired`          | The credentials have expired.                                    |
| `Timeout`          | Fetching credentials timed out.                                  |
| `Throttled`        | Credentials cannot be fetched again yet. Retry later.            |
| `Provider`         | The credential source failed. See the error message for details. |

## Related documents

- [Examples](examples.md)
- [Configuration](configuration.md)
