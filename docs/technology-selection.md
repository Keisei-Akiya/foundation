# 技術選定：言語の役割

このリポジトリで使う言語と道具が、それぞれ何を担当するかを記録する。
新しいコードを書くときに、どの言語に置くかを迷わないための基準である。

## 基本方針

判断のロジックは Rust の `foundation` バイナリに置く。
Python は、Hermes に読み込ませるための薄い橋渡しだけに使う。
この分け方は `adapters/hermes/plugin/__init__.py` の冒頭に書かれている。

> runs the `foundation` binary as a subprocess and converts its stdout into the return value Hermes expects. The binary stays the single source of hook logic.

## 言語ごとの役割

### Rust（`src/`）

担当：hook の判断ロジック、状態ストア、`foundation` コマンド本体（`enable`、`decide` など）、pre-commit の整形と検査。

選ぶ理由：
- 配布物が単一の実行ファイルになる。
  CI は `x86_64-unknown-linux-musl` 向けにビルドしており、インストール先の環境に実行時の依存を求めない。
- Hermes の plugin 形式（Python）にも、shell hook 形式にも、同じ判断を返せる。
  判断ロジックが一か所にあるので、呼び出し口が増えても挙動がずれない。
- `cargo test --locked`、`clippy -D warnings`、`cargo fmt --check` を CI の必須検査にしており、判断ロジックを型とテストで固められる。

置かないもの：Hermes の内部 API に依存する処理。

### Python（`adapters/hermes/plugin/`、`tests/`）

担当：
- Hermes plugin の入口（`plugin.yaml` と `__init__.py`、`register(ctx)`）。
- Hermes のイベントを受け取り、`foundation` バイナリを subprocess で呼び、結果を Hermes が期待する戻り値に変換する処理。
- Hermes の実物に対する結合テスト（`tests/`）。

選ぶ理由：
- Hermes の plugin は Python モジュールとして読み込まれる。
  この入口を別の言語で書くことはできない。
- shell hook では届かないイベントがある。
  Hermes が block を受け付けるのは `pre_tool_call` だけで、他のイベントで何かをさせるには plugin 経由になる。
- 結合テストは Hermes 自身の `hermes_cli.plugins.invoke_hook` と `VALID_HOOKS` を import する。
  Hermes の実装を直接使うため、テストも Python になる。

置かないもの：判断ロジック。
Python 側で分岐を増やすと、shell hook 形式との挙動差が生まれる。
条件の判定は Rust 側に足し、Python は引数の組み立てと戻り値の変換に留める。

### uv（Python の実行環境）

担当：Python のテストを実行するときの、使い捨ての環境づくり。

選ぶ理由：
- テストに必要なのは `pytest` だけで、Python の実行環境と依存は Hermes のものを使う。
  `uv run --with pytest --python "$HERMES_PYTHON"` なら、リポジトリに `pyproject.toml` や仮想環境を持たずに済む。
- `mise run runtime-test` と CI の両方が、すでにこの形で使っている。

決まり：
- このリポジトリで uv を使ってよい。
- Python の依存をリポジトリに固定する `pyproject.toml` は、現時点では置かない。
  Hermes 側の環境が依存の正であり、二重管理を避けるためである。
- Python パッケージとして配布する必要が出たときに、この決まりを見直す。

### シェル（`install.sh`、`scripts/`）

担当：インストーラー、リリース成果物の作成、version 更新の検査。

選ぶ理由：
- `install.sh` は `curl ... | sh` で実行されるため、`sh` で動く必要がある。
  CI は `sh -n install.sh` で構文を確認している。
- `scripts/` は `bash` を前提にしており、CI で `bash -n` により構文を確認している。
- 内容はファイルの配置、tar の作成、`git diff` の判定といった、コマンドを並べるだけの処理に限る。

置かないもの：条件分岐が増えた判断。
複雑になったら Rust に移す。

### YAML（`plugin.yaml`、`hooks.example.yaml`、`.github/workflows/`）

担当：Hermes への plugin 宣言、shell hook の登録例、CI の定義。
設定を書くだけで、ロジックは書かない。

### Markdown（`docs/`、`skills/`、`adapters/**/README.md`）

担当：設計の記録、利用手順、スキル定義。
`skills/` の `SKILL.md` は、エージェントが読む手順書として配布物に含まれる。

## 道具

- mise：道具の version とタスク（`mise run check`、`mise run runtime-test`）を一か所にまとめる。
- lefthook：commit 前の検査を呼び出す。
  実際の整形と検査は Rust の `precommit` バイナリが行う。

## 言語を足すときの基準

1. まず Rust で書けるかを考える。
2. Hermes の仕組み上、Python でしか書けない部分だけを Python にする。
3. ファイルの配置やコマンドの連結だけなら、シェルでよい。
4. 上のどれにも当てはまらない言語を足すときは、この文書に理由を追記してから入れる。
