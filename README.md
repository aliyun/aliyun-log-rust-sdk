

## compile

```bash
cargo build --release
```

## test

1. first, create a `.env` file in the root directory, replace the value with your own.

    ```ini
    ACCESS_KEY_ID=xxxx
    ACCESS_KEY_SECRET=xxx
    PROJECT=xxxxx
    LOGSTORE=xxxx
    ENDPOINT=cn-hangzhou.log.aliyuncs.com
    ```

2. run test

    ```bash
    cargo test
    ```

## publish

1. first, login to crates.io

    ```bash
    cargo login --registry=crates.io
    ```

2. cd the dir of a crate, and do publish

    ```bash
    cd client
    cargo publish
    ```

## document

```bash
cargo doc --open
```

## format
```bash
cargo fmt
```


## road to v1.0.0
todo:
- add comments and documents
- add api
  - get_cursor_time
  - consumer_group api
  - project api
  - logstore api
  - index api
- add samples
- add benchmarks