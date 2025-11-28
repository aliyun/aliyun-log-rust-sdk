# Changelog

## [0.2.0]

### Added

- **Project Management APIs**: Complete support for project management
  - `create_project` - Create a new project with configuration options
  - `update_project` - Update project description and settings
  - `delete_project` - Delete an existing project
  - `get_project` - Get detailed project information
  - `list_projects` - List projects with pagination and filtering support

- **Logstore Management APIs**: Complete support for logstore management
  - `create_logstore` - Create a new logstore with shard count, TTL, and advanced configurations
  - `update_logstore` - Update logstore settings including TTL, encryption, and storage tiers
  - `delete_logstore` - Delete a logstore and all its data
  - `get_logstore` - Get detailed logstore information and configuration
  - `list_logstores` - List logstores with pagination and filtering support

- **Index Management APIs**: Complete support for logstore index configuration
  - `create_index` - Create index configuration with full-text and field indexes
  - `update_index` - Update index configuration for better query performance
  - `delete_index` - Delete index configuration
  - `get_index` - Get current index configuration details

- **Consumer Group APIs**: Complete support for consumer group management
  - `create_consumer_group` - Create a consumer group for coordinated log consumption
  - `update_consumer_group` - Update consumer group configuration
  - `delete_consumer_group` - Delete a consumer group
  - `list_consumer_groups` - List all consumer groups in a logstore
  - `consumer_group_heartbeat` - Send heartbeat to maintain shard ownership
  - `get_consumer_group_checkpoint` - Get consumption checkpoints
  - `update_consumer_group_checkpoint` - Update consumption progress

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
