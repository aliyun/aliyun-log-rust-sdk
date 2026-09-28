# SLS Producer for Rust

[简体中文](README_CN.md)

Send logs to Alibaba Cloud Simple Log Service (SLS), with automatic batching,
retries, and callbacks to check delivery results.

| Guide | Contents |
| --- | --- |
| [Quick start](docs/quickstart.md) | Install, create a producer, send a log and close |
| [Usage examples](docs/examples.md) | Callbacks, flush, shutdown, timestamps and multiple logstores |
| [Configuration](docs/configuration.md) | Options, defaults and valid values |
| [Metrics](docs/metrics.md) | Track successful sends, failures, and log size |

The Producer starts when you create it. Reuse it and its writers while your application runs, and close it before exiting.

Logs are sent in the background. Use a callback to check whether delivery succeeded. Logs may arrive out of order or more than once. Pending logs may be lost if the application crashes.

[Python guide](../bindings/python/README.md)
