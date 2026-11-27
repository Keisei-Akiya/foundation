# Choice

Choice は順序のない有限の選択肢から一つを選ぶ分類に使う。
選択肢名だけで意味が明確でなければ説明を付ける。

## 使う場面

- 向く: 順序のない有限の選択肢から一つを選ぶ分類に使う。
- 向かない: 分類軸や選択肢の意味がわからない問いには使わない。

## 設計の要点

- 似た項目の境界をはっきりさせる。
- Choice は必ず一つを選ぶため、候補が入力を網羅しない可能性がある場合は、該当範囲を確かめる Noul を別に設計する。
- 「どう分類するか」のように、分類軸や選択肢の意味がわからない問いは曖昧である。

## 例

質問:
「問い合わせ本文が主に求める対応を、最も適切な担当先に分類せよ」

選択肢:
- `billing`: 請求、支払い、返金の処理を担当する。
- `returns`: 返品または交換の受付と処理を担当する。
- `shipping`: 未着、遅延など配送状況の確認や配達を担当する。

網羅性を確認する Noul の質問:
「問い合わせ本文の主な要件は、この3担当のいずれかが扱う内容か」

`questions` の一要素（`criteria` は選択肢名から説明へのマップ）:

```json
{
  "department": {
    "type": "choice",
    "instructions": "問い合わせ本文が主に求める対応を、最も適切な担当先に分類せよ",
    "criteria": {
      "billing": "請求、支払い、返金の処理を担当する。",
      "returns": "返品または交換の受付と処理を担当する。",
      "shipping": "未着、遅延など配送状況の確認や配達を担当する。"
    }
  }
}
```

`answers` の一要素（`choice` は確率最大の選択肢、`probabilities` の合計は 1。値は説明用）:

```json
{
  "department": {
    "type": "choice",
    "choice": "returns",
    "confidence": 0.93,
    "probabilities": { "billing": 0.03, "returns": 0.95, "shipping": 0.02 }
  }
}
```

## 一次情報

- [Choice](https://docs.typesafe.ai/primitives/choice): 選択肢の定義と回答フィールド。
- [Primitives (Questions)](https://docs.typesafe.ai/primitives): 型の選択と Choice の網羅性に関する案内。
