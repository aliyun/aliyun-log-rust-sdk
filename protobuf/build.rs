fn main() {
    prost_build::compile_protos(&["src/logs.proto"], &["src/"]).unwrap();
}
