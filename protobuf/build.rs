fn main() {
    if std::env::var("CARGO_CFG_TEST").is_ok() {
        prost_build::compile_protos(&["src/logs.proto"], &["src/"]).unwrap();
    }
}
