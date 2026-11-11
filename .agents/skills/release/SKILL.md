---
name: release
description: Use when a change to this repo should reach users. Bump the version and let CI publish.
---

# release

リリースはmainへのマージで自動的に行われる。
手動でタグをpushしない。
version更新の要否は判断せず、下の基準で機械的に決める。

## 基準

次のパスを変更するPRでは、versionを上げる。

- `src/`
- `adapters/`
- `skills/`
- `install.sh`
- `scripts/package-release.sh`
- `Cargo.lock`

上記に触れないPR（`docs/`、`tests/`、`.github/`、README、AGENTS.md のみ等）では上げない。
迷ったら上げる。
この基準は `scripts/check-version-bump.sh` が実装しており、CIがPRで検査する。
基準を変えるときは、このファイルとスクリプトの `dist_paths` を同時に直す。

## 手順

1. `Cargo.toml` の `version` を上げる。0.0.x の間はpatchだけを上げる。
2. `Cargo.lock` の `foundation` のversionも同じ値にそろえる。
3. READMEと `install.sh` の冒頭コメントの例にあるversion（`--version X.Y.Z`）も新しい値に更新する。
4. mainへマージしたら、`release` workflowの成功と `gh release view v<version>` で成果物の存在を確認する。
