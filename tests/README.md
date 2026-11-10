# tests

Rust CLI acceptance tests live in `hooks_cli.rs` and `config_protection_cli.rs`. They exercise representative JSON stdin/stdout contracts and selected protection cases; they do not mechanically port all former Python tests or verify live Hermes wiring. Run with `cargo test --locked` or `mise run test`.
