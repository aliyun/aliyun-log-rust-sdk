# Changelog

## [0.2.0]

### Added

- **Consumer Group APIs**: Complete support for consumer group management
  - `create_consumer_group`
  - `update_consumer_group`
  - `delete_consumer_group`
  - `list_consumer_groups`
  - `consumer_group_heartbeat`
  - `get_consumer_group_checkpoint`
  - `update_consumer_group_checkpoint`

### Changed

- Parameter validation now provides clearer error messages indicating which parameter is missing

## [0.1.0]

### Added

- Basic Log Service operations:
  - `put_logs()`: Write logs to a logstore
  - `put_logs_raw()`: Write raw log data
  - `get_logs()`: Query logs with filtering and time range support
  - `pull_logs()`: Pull logs from a specific shard
  - `get_cursor()`: Get cursor position for log consumption
  - `list_shards()`: List all shards in a logstore

[0.1.0]: https://github.com/aliyun/aliyun-log-rust-sdk/releases/tag/v0.1.0
