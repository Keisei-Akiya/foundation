# foundation

ECCからHermesへのhook移植調査、各種アダプター、テスト、ドキュメントを整理して管理する。対応範囲の根拠は `adapters/hermes/hooks/INVENTORY.md` のECC 27 registration調査を参照。調査時の全件を移植したとは主張しない。

## インストール

Linux x86_64 のみ対応する。
GitHub Releases のビルド済み成果物を取得し、SHA256を検証してから `<HERMES_HOME>`（既定 `~/.hermes`）に配置する。

```sh
curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh
```

バージョンを固定する場合は `sh -s -- --version 0.1.0` を付ける。
配置先は次のとおり。

- `<HERMES_HOME>/bin/ecc-hermes-hooks`
- `<HERMES_HOME>/foundation/`（adapters、skills、docs、VERSION）

インストーラーは `config.yaml`、hook許可設定、起動中のgatewayを変更しない。
hookを有効にするには `<HERMES_HOME>/foundation/adapters/hermes/hooks/INSTALL.md` を確認し、`hooks.example.yaml` から必要なentryだけを明示的に選択する。

Rustがある環境では次でも導入できる。この方式はバイナリのみを配置し、adapters等は含まない。

```sh
cargo install --git https://github.com/Keisei-Akiya/foundation --locked --bin ecc-hermes-hooks
```

## リリース手順

1. `Cargo.toml` の `version` を更新してmainへマージする。
2. `v<version>` のタグをpushする。
3. `release` workflowが、タグとCargo.tomlのversion一致確認、テスト、muslビルド、tar.gzとSHA256の生成、インストーラーのスモークテスト、Release公開を行う。

## Rust 開発・テスト

Rustup/Cargoは既存の管理環境を利用する。mise toolsではRustをインストールしない。Python製hook、Python依存・実行設定は本リポジトリから撤去済み。

```sh
cargo build --release
mise run test
mise run check
```

`mise run check`はRustテスト、Clippy、format checkを作業ツリーに対して非変更で実行する。Hermes hookは `adapters/hermes/hooks/hooks.example.yaml` を確認し、必要なentryのみ明示的に選択する。実Hermes接続は未検証。

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
