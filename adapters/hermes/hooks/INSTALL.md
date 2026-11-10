# Install and opt in

1. Install Rustup/Cargo through your normal Rust toolchain management; this repository does not install Rust or Python runtimes.
2. From the repository root run `cargo build --release`.
3. Open `hooks.example.yaml` and copy only the hook entries you have reviewed into `~/.hermes/config.yaml`. Replace `/FOUNDATION_ROOT` with this checkout's absolute path. Do not copy the comments/placeholders verbatim.
4. Use exactly one selected event/command pair; each hook has distinct behavior and data access. Sample config is not automatically installed.
5. Validate the argv command and payload contract with the Rust CLI. No Hermes live connection test is claimed.

Lifecycle tracking remains off unless the hook process explicitly receives `ECC_HERMES_SKILL_TRACK=1` and `HERMES_HOME`. It writes only validated identifier, version and normalized outcome to `$HERMES_HOME/plugin-data/ecc-hooks/skill-runs.jsonl`; avoid enabling where that metadata is not appropriate.

To run the repository checks, use `mise run check`. This validates the whole working tree without modifying it.

Lefthookの `pre-commit` は `mise run precommit` を実行する。
この処理は、stagedされた `.rs` ファイルを整形し、staged状態のindexを検査する。
作業ツリーの未stage変更はcommitに取り込まない。
Python製のstaged fixerは削除済みであり、Rust製ヘルパー `src/bin/precommit.rs` に置き換えられている。
