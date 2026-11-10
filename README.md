# foundation

skills、各種アダプター、テスト、ドキュメントを整理して管理する。
各領域の具体的な内容や対応範囲は今後定める。

## Python 開発・テスト環境

Python テストの実行には Python 3.10 以上と [uv](https://docs.astral.sh/uv/) が必要だ。開発用依存関係を同期し、テストを実行するには次を使う。

```sh
uv sync
uv run pytest -q
```

pytest と ruamel.yaml は開発・テスト用の依存関係として管理し、`uv.lock` で固定する。Python は開発時および Python 製フックを使う場合に必要だ。Rust 製フックだけを使う場合、Python 環境や uv は不要だ。

## mise コマンド

mise を使うと Python と Rust のテストや Rust のフォーマットをまとめて実行できる。必要なツール（mise、uv、Python、Rust）は事前に用意する。

```sh
mise run python-test # Python テストを実行する
mise run rust-test   # Rust テストを実行する
mise run test        # Python と Rust のテストを実行する
mise run check       # 全テストと Rust のフォーマットを検証する
mise run fmt-check   # Rust のフォーマットだけを検証する
```

Python テストは `uv.lock` を変更しないよう `--locked` で実行する。
`mise run test` と `mise run check` は各タスクを逐次実行する。
Python テスト中に Cargo が起動する場合もあるため、並列実行を避ける。
Cargo は標準のターゲットディレクトリを使い、環境固有のパスを固定しない。

mise は設定済みの Lefthook を `mise install` で用意するが、Python、uv、Rust toolchain はインストールしない。Rust は既存の Rust 環境を使う。

## Pre-commit checks

コミット前に Lefthook は Git index の一時スナップショット上で検査する。Ruff は stage 済み `.py` のみ lint（`--fix`）と format を実行し、rustfmt は stage 済み `.rs` のみを Cargo.toml と同じ edition 2021 で修正する（`skip_children` を有効化）。その後、スナップショット全体に `cargo clippy --all-targets --all-features --locked -- -D warnings` と `cargo fmt --all -- --check` を実行し、Python lint/format も再検査する。どの検査に失敗しても index へ修正を反映せずコミットを拒否する。修正対象と index 更新は stage 済みソースだけで、修正前に作業ツリーと index が一致していたファイルだけ作業ツリーへ修正を反映し、部分 stage ファイルの未 stage 内容は保持する。Rust 検査は実リポジトリの `target` をビルドキャッシュとして再利用する。独立の `mise run lint-check` と `mise run fmt-check` は作業ツリー全体を検査する。lint は全 Python ファイルの Ruff lint と Rust Clippy、format は Rust 全体とこのフックスクリプトの Ruff format を検査する（既存 Python 全体の format baseline は対象外）。`mise run fix-checks` は pre-commit と同じ隔離 snapshot 検査を実行する。

初回は mise、uv、既存の Rust toolchain、Python 3.10 以上を用意したうえで、次を実行する。

```sh
mise install  # Lefthook をインストール（Rust toolchain は mise で管理しない）
uv sync        # Python 開発依存を同期
lefthook install
```

フックを介さず同じ検査を実行するには `mise run lint-check` と `mise run fmt-check` を使う。`mise run check` はテスト、lint、format の全検査を逐次実行する。
