# foundation

## これは何か

リポジトリ全体で一つのHermesプラグインである。
ECCからHermesへ移植したhook、アダプター、スキル、テスト、ドキュメントを一式として管理し、一つのバージョンで配布する。
Linux x86_64 のみ対応する。

## 作る目的

ECCのhookをHermesで使えるようにし、Hermes上の作業を支えるスキルやコマンドとあわせて、一つのプラグインとして導入・更新できるようにする。
対応範囲の根拠は [INVENTORY](adapters/hermes/hooks/INVENTORY.md) のECC 27 registration調査にある。
調査時の全件を移植したとは主張しない。

## ドキュメント

### 使う

- [インストールと更新](docs/install-and-update.md)
- [実リリース向け手動スモークチェックリスト](docs/update-release-smoke-checklist.md)
- [wiki-search のログ](docs/wiki-search-log.md)
- [セッションコストレポート](docs/cost-tracker.md)

### 開発する

- [開発・リリース](docs/development.md)
- [技術選定：言語の役割](docs/technology-selection.md)
- [Hermes runtime integration tests](docs/runtime-testing.md)
- [テスト](tests/README.md)
- [intake-hint 計測](tools/intake-hint/README.md)

### 設計を知る

- [リポジトリの前提](AGENTS.md)
- [用語集](CONTEXT.md)
- [Hermesネイティブ plugin hook API と ECC 対応](docs/hermes-plugin-api.md)
- [Hook共通状態ストア](docs/state-store.md)
- ADR
  - [0001 Wiki検索の設計](docs/adr/0001-wiki-search-decisions.md)
  - [0002 foundation update](docs/adr/0002-update-command.md)
  - [0003 intake-hint](docs/adr/0003-intake-hint.md)

### 構成要素

- [adapters](adapters/README.md)
  - [Hermes adapter](adapters/hermes/README.md)
  - [Hermes hooks (Rust)](adapters/hermes/hooks/README.md)
  - [hooks_ext](adapters/hermes/plugin/hooks_ext/README.md)
  - [Upstream attribution](adapters/hermes/hooks/UPSTREAM.md)
  - [Claude Code adapter](adapters/claude-code/README.md)
- [skills](skills/README.md)
  - [decide-cli](skills/decide-cli/SKILL.md)
  - [decision-model-knowledge](skills/decision-model-knowledge/SKILL.md)
  - [intake-hint](skills/intake-hint/SKILL.md)
  - [review-hint](skills/review-hint/SKILL.md)
  - [wiki-search](skills/wiki-search/SKILL.md)
