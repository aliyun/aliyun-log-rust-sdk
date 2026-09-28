# SLS Producer for Rust

[简体中文](README_CN.md)

Send logs to Alibaba Cloud Simple Log Service (SLS), with automatic batching,
retries and optional delivery callbacks.

| Guide | Contents |
| --- | --- |
| [Quick start](docs/quickstart.md) | Install, create a producer, send a log and close |
| [Usage examples](docs/examples.md) | Callbacks, flush, shutdown, timestamps and multiple logstores |
| [Configuration](docs/configuration.md) | Options, defaults and valid values |
| [Metrics](docs/metrics.md) | Metric names, units and recording semantics |

Create a producer for your endpoint and credentials, then obtain a writer for each
project/logstore. Creation starts the producer; no separate `start()` call is needed.
Reuse the producer and writers for subsequent sends.

A successful `send` means the log was accepted locally. Use a callback to learn the
final delivery result. Before exiting, close the producer to wait for pending logs
and callbacks. Delivery order is not guaranteed, and retries may produce duplicates.
Pending logs are not persisted to disk.

[Python guide](../bindings/python/README.md)
