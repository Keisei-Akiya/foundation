---
name: decision-model-design
description: Use when designing state and typed questions for a decision model.
---

# decision-model-design

目的から評価対象の `state` と型付き質問を設計し、広い依頼を小さく自己完結した判断へ分ける。
CLI の spec 形式・操作は `decide-cli` を参照し、このスキルでは重複して説明しない。

## 共通原則

- ソフトウェアが担う制御フローと、モデルに任せる曖昧な判断を分ける。
- `state` には判断に必要な情報だけを含め、各質問が単独で答えられる文脈を書く。
- 一つの質問で一つの判断を聞き、他の質問への回答や会話上の「上記」などに依存させない。
- 同じリクエスト内の質問は同じ `state` から独立して評価される。依存する判断は別途設計する。
- 回答の利用方法、閾値、ポリシー、後続処理はアプリケーション側で決める。

## 型の選択

| 型 | 問いの形 |
| --- | --- |
| Noul | はい／いいえで答えられる命題 |
| Choice | 順序のない有限の選択肢から一つ選ぶ分類 |
| Score | 説明可能な順序尺度上の位置 |

型を決めたら、必要な参照だけを読む。

- Noul の質問を作るときは [references/noul.md](references/noul.md) を参照する。
- Choice の選択肢を作るときは [references/choice.md](references/choice.md) を参照する。
- Score の尺度を作るときは [references/score.md](references/score.md) を参照する。

`decision-model-knowledge` は判断モデルや型の概念を説明し、`decide-cli` は共通 spec と CLI 操作を説明する。
このスキルは質問設計に限り、精度評価手順や閾値を定めない。
