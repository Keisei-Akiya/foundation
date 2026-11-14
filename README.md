# foundation

`foundation` は、リポジトリ全体で一つのHermesプラグインである。
ECCからHermesへのhook移植、アダプター、スキル、テスト、ドキュメントを一式として管理し、一つのバージョンで配布する。
`adapters/`、`skills/`、`docs/`、バイナリはその構成要素であり、個別の製品ではない。
対応範囲の根拠は `adapters/hermes/hooks/INVENTORY.md` のECC 27 registration調査を参照。
調査時の全件を移植したとは主張しない。

## インストール

Linux x86_64 のみ対応する。
GitHub Releases のビルド済み成果物を取得し、SHA256を検証してから `<HERMES_HOME>`（既定 `~/.hermes`）に配置する。

```sh
curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh
```

バージョンを固定する場合は `sh -s -- --version 0.0.14` を付ける。
配置先は次のとおり。

- `<HERMES_HOME>/bin/foundation`
- `<HERMES_HOME>/foundation/`（adapters、skills、docs、VERSION）

インストーラーはネイティブpluginを既定で有効にする。
`config.yaml`、hook許可設定、起動中のgatewayは変更しない。
shell hooksも有効にする場合だけ `--enable-hooks` を指定する。

残るshell hooksを自動で有効にする場合は `--enable-hooks` を付ける（任意）。

```sh
curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh -s -- --enable-hooks
```

導入済みなら `<HERMES_HOME>/bin/foundation enable` だけでもよい。
`--dry-run` を付けると変更内容の表示のみで書き込まない。
`enable` は次の動作をする。

- `config.yaml` を `config.yaml.bak-<時刻>` にバックアップしてから、末尾に `hooks:` ブロック（4件）を追記する。
- `shell-hooks-allowlist.json` に同じ4件の `(event, command)` だけを承認として記録する。`hooks_auto_accept` は設定しない。
- `config.yaml` に既に `hooks:` セクションがある場合は、その中へ不足している hook だけを追加する。既存の記述・コメントは変更しない。
- `hooks:` が `*anchor` やリスト形式などで安全にマージできない場合は、何も変更せずに終了する。その場合は手動で `hooks.example.yaml` を反映する。
- 何度実行しても結果は同じ（冪等）。gatewayの再起動は不要で、実行中のgatewayは約30秒で反映する。
- 反映の確認は `hermes hooks doctor` で行う。

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

`mise run check`はRustテスト、Clippy、format checkを作業ツリーに対して非変更で実行する。残るHermes shell hooksは `adapters/hermes/hooks/hooks.example.yaml` を確認し、必要なentryのみ明示的に選択する。
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
