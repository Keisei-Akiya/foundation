# foundation

`foundation` は、リポジトリ全体で一つのHermesプラグインである。
ECCからHermesへのhook移植、アダプター、スキル、テスト、ドキュメントを一式として管理し、一つのバージョンで配布する。
`adapters/`、`skills/`、`docs/`、バイナリはその構成要素であり、個別の製品ではない。
対応範囲の根拠は `adapters/hermes/hooks/INVENTORY.md` のECC 27 registration調査を参照。
調査時の全件を移植したとは主張しない。
Rust、Python、uv などの言語の役割と選ぶ理由は [技術選定](docs/technology-selection.md) を参照。

## インストール

Linux x86_64 のみ対応する。
GitHub Releases のビルド済み成果物を取得し、SHA256を検証してから `<HERMES_HOME>`（既定 `~/.hermes`）に配置する。

```sh
curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh
```

バージョンを固定する場合は `sh -s -- --version 0.0.18` を付ける。
配置先は次のとおり。

- `<HERMES_HOME>/bin/foundation`
- `<HERMES_HOME>/foundation/`（adapters、skills、docs、VERSION）

インストーラーはHermes CLIを使ってネイティブpluginを有効にする。
`config.yaml`、hook許可設定、起動中のgatewayは変更しない。
Hermes CLIが見つからない場合は、配置を完了した後にエラーで停止する。
プラグインを無効にする場合は `hermes plugins disable foundation` を実行する。

Rustがある環境では次でも導入できる。この方式はバイナリのみを配置し、adapters等は含まない。

```sh
cargo install --git https://github.com/Keisei-Akiya/foundation --locked --bin foundation
```

## リリース手順

1. `Cargo.toml` の `version` を更新してmainへマージする。
2. `release` workflowがmainへのpushで動き、`v<version>` のタグが無い場合だけリリースする。
3. リリースではテスト、muslビルド、tar.gzとSHA256の生成、インストーラーのスモークテスト、Release公開を行い、タグも自動で付く。

versionを上げずにマージした場合は、タグが既にあるためリリースされない。
手動でタグをpushする必要はない。

## Rust 開発・テスト

Rustup/Cargoは既存の管理環境を利用する。mise toolsではRustやPythonをインストールしない。Hermes実行環境用のPythonはruntime test実行時に指定する。

```sh
cargo build --release
mise run test
mise run check
```

`mise run check`はRustテスト、Clippy、format checkを作業ツリーに対して非変更で実行する。
Hermes実行環境がある場合は `HERMES_PYTHON=/path/to/hermes/python mise run runtime-test` で、隔離 `HERMES_HOME` におけるネイティブpluginのHermes hook dispatcher接続も検査する。手順と制約は `docs/runtime-testing.md` を参照。

`mise run precommit`はヘルパーを作業ツリーからビルドして起動する。
そのため、ヘルパー自身の未コミット変更はそのhookでは検証されていない。
ヘルパーを変更するコミットでは、この点を確認してから結果を信頼する。

## Pre-commit

Lefthookの `pre-commit` は `mise run precommit` を実行する。
このtaskは `cargo run --quiet --locked --bin precommit --` で `src/bin/precommit.rs` のヘルパーを起動する。
ヘルパーは次の処理を行う。

- stagedされた `.rs` ファイルのみを `cargo fmt` で整形する。
- 整形結果はindexのblobにのみ反映し、partial-stageされたファイルの作業ツリーには触れない。
- 作業ツリーがindexと一致していたstagedファイルのみ、整形後の内容を作業ツリーにも書き戻す。
- 検査は、staged状態のindexを一時ディレクトリへ展開したsnapshotに対して実行する。
- 検査は `cargo fmt --all -- --check` と `cargo clippy --all-targets --all-features --locked -- -D warnings` である。
- 作業ツリーのみにある違反は検査対象外となる。
- index上の違反は、stagedかどうかにかかわらずcommitを止める。

いずれかの検査が失敗した場合、indexと作業ツリーは変更されず、commitは中止される。
ヘルパーはUnix系環境を前提とする。
ビルドキャッシュには `CARGO_TARGET_DIR` を使い、未設定の場合は `target/` を使う。

`foundation wiki-search` appends one JSONL record per invocation to `$XDG_STATE_HOME/foundation/wiki-search.jsonl` (or `~/.local/state/foundation/wiki-search.jsonl` when `XDG_STATE_HOME` is unset). Override the destination with `--log-path <file>`. The log contains input text, domain/page scores, returned page paths and Jev usage; page bodies are not logged.
