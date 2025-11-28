API 参考
========

`English <api.rst>`_ | 简体中文

本页面提供了阿里云日志服务 Rust SDK 中可用 API。


Client 配置
-------------

有关客户端配置选项，请参阅 `Client <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html>`_ 与 `Config <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Config.html>`_ 文档。


项目管理
--------

用于管理日志项目的 API，项目是日志服务的最顶层资源单元。

* `create_project <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.create_project>`_ - 创建新的项目，项目名称必须全局唯一
* `update_project <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.update_project>`_ - 更新项目描述和回收站等配置
* `delete_project <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.delete_project>`_ - 删除项目及其所有相关资源
* `get_project <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.get_project>`_ - 获取项目的详细信息，包括创建时间、状态等
* `list_projects <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.list_projects>`_ - 列出所有项目，支持分页和按名称、描述、资源组等过滤

Logstore 管理
-------------

用于管理 Logstore 的 API，Logstore 是日志存储、查询和分析的单元。

* `create_logstore <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.create_logstore>`_ - 创建新的 Logstore，支持配置分片数、TTL、加密、自动分裂等
* `update_logstore <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.update_logstore>`_ - 更新 Logstore 的配置，如 TTL、加密、热存储等
* `delete_logstore <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.delete_logstore>`_ - 删除 Logstore 及其所有数据
* `get_logstore <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.get_logstore>`_ - 获取 Logstore 的详细信息，包括配置和统计数据
* `list_logstores <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.list_logstores>`_ - 列出项目中的所有 Logstore，支持分页和按名称、类型等过滤


日志操作
--------

用于向日志库写入和查询日志的 API。

* `put_logs <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.put_logs>`_ - 使用 Protocol Buffer 格式向日志库写入日志
* `put_logs_raw <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.put_logs_raw>`_ - 使用自定义压缩方式向日志库写入原始日志数据
* `get_logs <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.get_logs>`_ - 从日志库查询某一时间范围内的日志，支持使用查询或 sql 等语法
* `pull_logs <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.pull_logs>`_ - 从特定 shard 分片拉取日志以进行消费
* `get_cursor <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.get_cursor>`_ - 获取从特定时间或位置的日志游标位置

分片管理
--------

用于管理和查询日志库分片的 API。

* `list_shards <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.list_shards>`_ - 列出日志库中的所有分片及其状态

消费组管理
----------

用于管理消费组的 API，消费组可实现多个消费者之间的协调日志消费。

消费组
~~~~~~~~~~~~~~

* `create_consumer_group <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.create_consumer_group>`_ - 创建新的消费组
* `update_consumer_group <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.update_consumer_group>`_ - 更新消费组设置，如超时和顺序消费配置
* `delete_consumer_group <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.delete_consumer_group>`_ - 删除消费组及其所有关联的消费位点
* `list_consumer_groups <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.list_consumer_groups>`_ - 列出日志库中的所有消费组及其配置

消费相关
~~~~~~~~~~

* `consumer_group_heartbeat <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.consumer_group_heartbeat>`_ - 发送心跳以维持分片所有权并获取分配的分片
* `get_consumer_group_checkpoint <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.get_consumer_group_checkpoint>`_ - 获取消费位点，即分片的消费进度
* `update_consumer_group_checkpoint <https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/struct.Client.html#method.update_consumer_group_checkpoint>`_ - 更新消费位点


