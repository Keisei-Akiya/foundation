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

バージョンを固定する場合は `sh -s -- --version 0.0.27` を付ける。
配置先は次のとおり。

- `<HERMES_HOME>/bin/foundation`
- `<HERMES_HOME>/foundation/`（adapters、skills、docs、VERSION）

インストーラーはHermes CLIを使ってネイティブpluginを有効にし、`config.yaml` の `plugins.enabled` を更新する。
それ以外の設定とhook許可設定は変更せず、起動中のgatewayも再起動しない。反映にはgatewayの再起動が必要である。
Hermes CLIが見つからない場合は、配置を完了した後にエラーで停止する。
プラグインを無効にする場合は `hermes plugins disable foundation` を実行する。

## バージョン確認と更新

導入済みのバイナリと共有ツリーの版は、`foundation version` で確認する。このコマンドはネットワークに接続しない。共有ツリーの `VERSION` ファイルが無い場合は、その版を `unavailable (VERSION file not found)` と表示する。

```sh
foundation version
```

最新版への更新の要否は `foundation update --check` で確認する。最新の版、バイナリの版、共有ツリーの版、および `Update available` または `Already up to date` が表示される。更新があっても終了コードは 0 であり、このコマンドはファイルを変更しない。

```sh
foundation update --check
```

最新版へ更新するには、次を実行する。

```sh
foundation update
```

すでに最新で、バイナリと共有ツリーの版も一致している場合は `最新です (version X.Y.Z)` と表示して何も変更しない。更新が成功すると `更新が完了しました。gateway の再起動が必要です。` と表示する。更新コマンドは gateway を再起動しないため、更新を反映するには利用者が gateway を再起動する必要がある。

特定の版へ更新するには `--version X.Y.Z` を指定する。指定した版が存在しない場合はインストーラーの取得が失敗し、エラーで終了する。既存のバイナリや共有ツリーはその場合変更されない。新しい版から以前の版へ戻す（ダウングレードする）場合も同じ指定を使う。

```sh
foundation update --version 0.0.27
```

`version` と `update` の対象 Hermes home は、`--hermes-home DIR`、環境変数 `HERMES_HOME`、既定の `~/.hermes` の順で決まる。別のプロファイルを対象にする場合は、次のように両コマンドで指定する。

```sh
foundation version --hermes-home /path/to/hermes-home
foundation update --check --hermes-home /path/to/hermes-home
foundation update --hermes-home /path/to/hermes-home
```

リリース後に実物で一度だけ確認する手順は、[実リリース向け手動スモークチェックリスト](docs/update-release-smoke-checklist.md) を参照する。

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
