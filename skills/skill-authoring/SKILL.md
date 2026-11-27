---
name: skill-authoring
description: Use when creating or revising a foundation skill. Search existing skills first.
---

# skill-authoring

スキルを作る前に、既にあるものを探す。
無いと確かめられたときだけ、書く。
汎用的なスキルの作り方（評価、description の最適化）は、公式の `skill-creator` が扱う。
このスキルは、foundation で守る差分だけを置く。

## 1. 作る前に探す

次の順に確認する。
どこかにあれば、それを使う。
足りない差分だけを、foundation に書く。

1. `hermes skills list`: すでに入れたスキル。
2. `hermes skills search <語>`: 公式と各レジストリ。`anthropics/skills`、`openai/skills`、`typesafe-ai/skills` など。
3. `skills/`: foundation にすでにあるスキル。

判断: 「探した結果を、一行で言えるか」。
言えないなら、まだ書かない。

## 2. 置く場所を決める

- 配置は `skills/<name>/SKILL.md`。
- foundation のスキルは、CLI の使い方だけを書く。
  環境、認証、一般原則は書かない。
  ツールのインストールは、別の経路で行う。
- 原則は一か所に置く。他のスキルは、それを参照する。

## 3. 短く、抽象から具体へ書く

- `description` は `Use when ...` で始め、使う場面を一文で書く。
- 本文は、原則、判断、具体の順に並べる。
- 長くなる分は、`references/` に分ける。
  SKILL.md には、分けた先の場所と、読む条件だけを書く。
- 原則や判断は、具体例か次の一手に落とす。

## 4. 出すとき

- `skills/README.md` に、一行を足す。
- バージョンの patch を上げる。
  `Cargo.toml` と、`Cargo.lock` の `foundation` を揃える。
- `adapters/hermes/plugin/plugin.yaml` は手で編集しない。
  lefthook の `sync-plugin-manifest` が生成する。

完了の判定: 手順 1 の確認結果を、PR 本文に書いた。
