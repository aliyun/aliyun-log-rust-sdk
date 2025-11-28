API Reference
=============

English | `简体中文 <api_cn.rst>`_

This page provides available APIs in the Aliyun Log Service Rust SDK.

Client Configuration
-----------------------

For client configuration options, see the `Client <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html>`_ and `Config <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Config.html>`_ documentation.


Log Operations
--------------

APIs for writing and querying logs from logstores.

* `put_logs <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.put_logs>`_ - Write logs to a logstore using Protocol Buffer format
* `put_logs_raw <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.put_logs_raw>`_ - Write raw log data to a logstore with custom compression
* `get_logs <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.get_logs>`_ - Query logs within a time range using query or SQL syntax
* `pull_logs <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.pull_logs>`_ - Pull logs from a specific shard for consumption
* `get_cursor <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.get_cursor>`_ - Get a cursor position from a specific time or location

Shard Management
----------------

APIs for managing and querying logstore shards.

* `list_shards <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.list_shards>`_ - List all shards in a logstore with their status

Consumer Group Management
-------------------------

APIs for managing consumer groups, which enable coordinated log consumption across multiple consumers.

Consumer Group
~~~~~~~~~~~~~~

* `create_consumer_group <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.create_consumer_group>`_ - Create a new consumer group
* `update_consumer_group <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.update_consumer_group>`_ - Update consumer group settings such as timeout and ordering
* `delete_consumer_group <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.delete_consumer_group>`_ - Delete a consumer group and all its associated checkpoints
* `list_consumer_groups <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.list_consumer_groups>`_ - List all consumer groups in a logstore with their configurations

Consumption
~~~~~~~~~~~

* `consumer_group_heartbeat <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.consumer_group_heartbeat>`_ - Send heartbeat to maintain shard ownership and get assigned shards
* `get_consumer_group_checkpoint <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.get_consumer_group_checkpoint>`_ - Get consumption checkpoint to track shard consumption progress
* `update_consumer_group_checkpoint <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.update_consumer_group_checkpoint>`_ - Update consumption checkpoint

