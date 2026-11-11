---
name: decision-model-knowledge
description: Use when deciding whether and how to use a decision model (Jev, Microsoft-Decision-1, OpenAI Decisions API). Gives the provider differences and question types.
---

# decision-model-knowledge

decision model は、文章を生成せず、与えた入力に対する構造化された判定を返すモデルである。
自由文の回答が要る用途には使わない。
分類・ルーティング・真偽判定・段階評価のように、答えの形が決まっている用途に使う。

## 提供元

- Jev は TypeSafe の System One 系モデルである。TypeSafe の公式ドキュメントでは、Jev を「TypeSafe's flagship model and the first System One model」と説明している。TypeSafe の API の `POST /v1/systemone` に `model` として `jev-latest` などを指定して呼び出す。Microsoft Foundry にデプロイして使うモデルではない。
- Microsoft-Decision-1 は Microsoft が Foundry のモデルカタログで公開している decision モデルであり、public preview である。Microsoft の発表によると Qwen3.5-9B をベースにしており、Jev とは別のモデルである。Foundry のリソースにデプロイし、`POST <resource-endpoint>/providers/microsoft/v1/systemone` で呼び出す。リクエストの `model` には基盤モデル名ではなくデプロイ名を指定する。認証には Microsoft Entra ID または `api-key` ヘッダーを使う。Microsoft Learn はリクエストパターンが System One API に似ていると説明しているが、提供元もモデルも Jev とは異なる。
- OpenAI Decisions API は `POST /v1/decisions` で呼び出す。公式ドキュメントによると、現時点で対応するモデルは `gpt-6-luna` のみである。質問の種類は `predicate`、`choice`、`score` である。

Jev の背景は wiki（`wiki/entities/jev.md`、`wiki/concepts/system-one-models.md`）を参照する。
仕様の一次情報は https://docs.typesafe.ai/concepts/system-one、https://learn.microsoft.com/en-us/azure/foundry/foundry-models/how-to/use-foundry-models-microsoft-decision、https://techcommunity.microsoft.com/blog/azure-ai-foundry-blog/introducing-microsoft-decision-1-in-microsoft-foundry-for-decision-and-classific/4562742、https://developers.openai.com/api/docs/guides/decisions である。
このスキルに書いていない詳細は推測せず、一次情報を確認する。
2026-11-12 に確認した。

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
