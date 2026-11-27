---
name: decision-model-design
description: Use when designing state and typed questions for a decision model.
---

# decision-model-design

目的から評価対象の `state` と型付き質問を設計し、広い依頼を小さく自己完結した判断へ分ける。
このスキルはモデル知識の説明や `foundation decide` の操作ではなく、質問設計を扱う。

## 設計手順

1. ソフトウェアが下す決定と、モデルに任せる曖昧な判断を分ける。
2. 判断に必要な入力だけを `state` に含め、質問単体で答えられる文脈を用意する。
3. 一つの質問で一つの判断だけを聞き、答えの形に合う型を選ぶ。
4. 質問文と選択肢・尺度を具体化し、各項目の意味が重ならないようにする。
5. 複数の質問を送る場合は、それぞれが同じ `state` から独立に答えられることを確認する。
6. 回答をどう使うかは通常のコードで定める。質問設計だけで閾値や後続処理を決めない。

## `state` と質問の共通原則

TypeSafe の API では、リクエストに評価対象の `state`、モデル、`questions` を渡す。
各質問には `instructions` を書き、質問が参照する対象や条件を明示する。
指示文が「上記」「その項目」のような会話文脈や別質問の回答を前提にしないようにする。
質問 ID は応答を対応付けるためのキーであり、モデルには送られない。
質問 ID に意味を込めてもモデルの判断材料にはならない。

広い目的は、モデルに任せたい小さな判断へ分ける。
たとえば「この問い合わせを処理せよ」ではなく、「返金を求めているか」「どの担当チームが扱う内容か」のように質問する。
ただし、質問を増やすだけで依存関係を表現できるわけではない。
一つのリクエスト内の複数質問は同じ `state` を受けて独立評価されるため、別の質問の結果を条件にした問いは作らない。

`state` には質問に必要な情報を含め、不要な情報を詰め込まない。
複数の質問が同じ状態を共有するため、ある質問だけに必要な情報を分離したい場合は、リクエストを分ける設計も検討する。

## 型の選び方

| 型 | 適する判断 | 入力定義 | 出力 |
| --- | --- | --- | --- |
| Noul | はい／いいえで答えられる命題 | `instructions` に自己完結した質問または判定文を書く。必要なら `criteria` に true / false の意味を定義する。 | `noul` は Yes である確率を表す 0〜1 の数値だけを返す。根拠文や説明は返さない。 |
| Choice | 順序のない有限の選択肢から一つ選ぶ | `instructions` と、各 option 名・説明を持つ `criteria` の map を定義する。似た選択肢は説明で境界を区別する。 | `choice`、各選択肢の `probabilities`、選ばれた選択肢の `confidence` を返す。 |
| Score | 言葉で説明できる順序尺度上の位置を評価する | `instructions` と、低い側から高い側へ並べた `criteria` の説明配列を定義する。尺度は2〜10段階にする。 | `score`、各レベルの `probabilities`、レベル説明の `legend`、`confidence` を返す。`score` はレベル間の値にもなる。 |

### `foundation decide` での定義例

TypeSafe の公式 API は `state` と質問 ID をキーにした `questions` を使い、Choice は `criteria` の map、Score は `criteria` の順序付き配列で定義する。
一方、`foundation decide` の共通 spec は `input` と質問の配列を使う。
TypeSafe provider の実装は `predicate` を Noul に変換し、`choice` と `score` を公式 API の型へ変換する。
したがって、下の JSON は CLI 用であり、公式 API のリクエスト形式そのものではない。

```json
{
  "input": "注文 #123 は必要期限の6月10日を過ぎても未着である。6月12日の出張で使う予定だったが、手元に代替品はない。注文代金の返金を希望する。",
  "questions": [
    {
      "name": "refund_requested",
      "type": "predicate",
      "instructions": "本文は注文代金の返金を明示的に求めているか"
    },
    {
      "name": "team",
      "type": "choice",
      "instructions": "問い合わせ本文が主に求める対応を担当先に分類する。返金・支払いの処理を求める場合は billing、配送状況の確認や配達を求める場合は shipping",
      "choices": [
        {"value": "billing", "description": "請求・支払い・返金の処理を担当する"},
        {"value": "shipping", "description": "未着・遅延など配送状況の確認や配達を担当する"}
      ]
    },
    {
      "name": "impact",
      "type": "score",
      "instructions": "未着による出張への影響を、必要期限と代替手段の有無に基づいて低い順で評価する",
      "levels": [
        {"label": "minor", "description": "必要期限まで余裕があり、出張で使う予定に支障がない"},
        {"label": "major", "description": "必要期限を過ぎるか予定に支障があるが、代替手段を利用できる"},
        {"label": "critical", "description": "必要期限を過ぎ、出張で使えず、代替手段もない"}
      ]
    }
  ]
}
```

`foundation decide` の `choice` は2個以上の `{value, description}` を要求する。
`score` は2個以上の `{label, description?}` を要求し、並び順が尺度の低い側から高い側になる。
TypeSafe provider で Choice と Score が変換されることは `src/decide/request.rs` に基づく。
OpenAI provider は共通 spec の型と `choices`／`levels` を保持するため、TypeSafe の API 形式とは区別する。

### Noul

Yes / No の判定に使い、複数カテゴリの分類には使わない。
公式 API の Noul 回答は `noul` という数値で、Yes の確率を示す。
理由や引用箇所が必要な要件を、Noul が説明まで返す前提で設計しない。

悪い例: 「この問い合わせは何についてか」。

改善例: 「問い合わせ本文は、注文代金の返金を明示的に求めているか」。

複数選択肢のどれかが該当するかも知りたい場合は、Choice と別の Noul に分ける。

### Choice

固定された選択肢から一つを選ぶ分類に使う。
選択肢名だけで意味が明確でなければ説明を付け、隣接カテゴリと区別できるようにする。
選択肢が入力を網羅しない可能性がある場合、Choice は必ず一つ選ぶため、公式の推奨に従い「どれかの選択肢が該当するか」を尋ねる Noul を併せる。
「その他」などを選択肢に加える設計は実例としてあり得るが、網羅性の判定には Noul を使う方法を優先する。

悪い例: 「どう分類するか」。選択肢の意味も網羅性も定義していない。

改善例: 「問い合わせ本文の主な要件を、最も適切な担当先に分類せよ」。選択肢を `billing`（請求・支払い）、`returns`（返品・交換）、`shipping`（配送状況・遅延）と定義し、別の Noul で「本文の要件はこの3担当のいずれかが扱う内容か」を尋ねる。

### Score

重大度、満足度、経験度など、段階の順序に意味がある尺度に使う。
単なる選択肢のリストには使わず、各レベルを低い順に並べて、隣り合う段階の境界がわかる説明を書く。
公式ガイドは最初に文字列の説明を使い、境界が曖昧な場合に対象範囲や実例を含む構造化説明へ進むことを勧める。
`score` はレベル番号上の位置であり、比率や事象の発生割合ではない。
確率分布や `confidence` も合わせて読む。

悪い例: レベルが `low`, `medium`, `high` だけで、何をもって段階を分けるか書いていない。

改善例: 不具合報告の重大度を「機能影響なし」「機能が壊れるが回避策がある」「機能を利用できず回避策もない」の順で評価する。

## 根拠と実例の区別

上記の型、フィールド、回答形状、独立評価、質問 ID の扱い、Choice の網羅性に関する勧めは TypeSafe 公式ドキュメントに基づく。
コード例のような問い合わせ分類、担当名、文言は、このスキルの設計例であり、TypeSafe が精度を保証した例ではない。
このスキルはモデル精度を検証したとは主張しない。

## 責務の境界

- `decision-model-knowledge/` は判断モデル、Jev、型の概念を説明する。
- `decide-cli/` は `foundation decide` の spec と CLI 操作を説明する。
- `decision-model-design/` は目的から `state`、質問、型、選択肢や尺度を設計する。

このスキルは閾値、質問の組み合わせ方、評価手順を一律に決めない。
精度評価、ポリシー、最終的な副作用や制御フローは、それらを所有するアプリケーション側の要件として扱う。
環境設定、認証、モデル精度の検証手順は扱わない。

## 一次情報

- [How to build with TypeSafe](https://docs.typesafe.ai/concepts/how-to-build-with-system-one): コードが制御フローを持ち、モデルには狭く構造化した判断を任せる設計。
- [Primitives (Questions)](https://docs.typesafe.ai/primitives): 型の選択、質問ごとの独立評価、Choice の網羅性。
- [Noul](https://docs.typesafe.ai/primitives/noul): Yes の確率を返す質問と回答。
- [Choice](https://docs.typesafe.ai/primitives/choice): 固定選択肢、回答フィールド、選択肢の定義。
- [Score](https://docs.typesafe.ai/primitives/score): 順序尺度、レベルの定義、回答フィールド。
