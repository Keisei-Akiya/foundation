---
name: release
description: Use when a change to this repo should reach users. Bump the version and let CI publish.
---

# release

リリースはmainへのマージで自動的に行われる。
手動でタグをpushしない。

1. 利用者に届けたい変更を含むPRでは、`Cargo.toml` の `version` を上げる。
2. `Cargo.lock` の `ecc-hermes-hooks` のversionも同じ値にそろえる。
3. 0.0.x の間はpatchだけを上げる。
4. 利用者に届ける必要がない変更（テストやCI設定のみ等）では上げない。
5. READMEの例にあるversion（`--version X.Y.Z`）も新しい値に更新する。
6. mainへマージしたら、`release` workflowの成功と `gh release view v<version>` で成果物の存在を確認する。
