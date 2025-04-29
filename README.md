

## compile

```bash
cargo build --release
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