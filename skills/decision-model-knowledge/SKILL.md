---
name: decision-model-knowledge
description: Use when deciding whether and how to use a decision model (Jev, Microsoft-Decision-1, OpenAI Decisions API). Gives the provider differences and question types.
---

# decision-model-knowledge

decision model は、文章を生成せず、与えた入力に対する構造化された判定を返すモデルである。
自由文の回答が要る用途には使わない。
分類・ルーティング・真偽判定・段階評価のように、答えの形が決まっている用途に使う。

## 提供元

- Jev は TypeSafe が提供する System One 系モデルである。Microsoft Foundry にデプロイされたモデルとして扱わない。
- Microsoft-Decision-1 は Jev と区別する。Microsoft Foundry 側ではデプロイ名を指定して呼び出す。
- OpenAI Decisions API: `POST /v1/decisions`。既定モデルは `gpt-6-luna`。

Jev の背景は jev wiki（`wiki/entities/jev.md`、`wiki/concepts/system-one-models.md`）を参照する。
仕様の一次情報は Microsoft Learn の Foundry decision 解説と OpenAI の Decisions API ドキュメントである。
このスキルに書いていない詳細は、推測せず一次情報を確認する。

## 質問の種類

共通 spec では3種類を使う。

- `predicate`: 真偽の判定。Microsoft 側では `noul` 型に変換される。
- `choice`: 選択肢から1つ選ぶ。各選択肢に `value` と `description` を付ける。
- `score`: 段階評価。`levels` に `label` を並べる。

## 使い分け

- 入力が構造化データ（JSON）でも文章でも渡せる。
- 質問は1回の呼び出しに複数まとめられる。名前は重複させない。
- `instructions` は判定基準を具体的に書く。曖昧だと判定がぶれる。
- 判定結果は材料として扱う。副作用のある操作を、判定だけで自動実行しない。

呼び出しは `foundation decide` を使う。手順は `decide-cli` スキルを参照する。
