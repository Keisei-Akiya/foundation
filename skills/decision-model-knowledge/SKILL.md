---
name: decision-model-knowledge
description: Use when explaining decision models, Jev, or typed judgments.
---

# decision-model-knowledge

判断モデル（TypeSafe の System One model）は、与えられた状態について、ソフトウェアが扱いやすい型付きの判定と確率を返すモデルである。
自由文の回答、コード、推論説明を生成する用途ではなく、焦点を絞った判断をアプリケーションの処理に組み込む用途に向く。
複雑な判断は小さな質問に分け、回答をアプリケーション側のロジックと組み合わせる。

## Jev

Jev は TypeSafe の旗艦モデルであり、最初の System One model である。
TypeSafe のモデル名・別名は更新されうるため、具体的なモデル ID や別名を前提にせず、必要なら公式モデル一覧を確認する。
Jev の公式説明では、入力された状態を評価して型付きの回答と確率を返す。
確率の較正は予測のグループ全体で評価されるもので、個々の回答が正しいことを保証しない。

## 質問と回答の型

TypeSafe の質問は `state` と質問ごとの `instructions` をもとに評価される。
複数の質問を一度に渡すと、各質問は同じ state を見て独立に評価される。

- **Noul**: 命題が真かを問う。
  `noul` は 0 から 1 の値で、命題が真である確率を表す。
- **Choice**: 定義した選択肢から選ぶ。
  回答には選ばれた値のほか、選択肢ごとの確率や confidence が含まれる。
- **Score**: 定義した順序付きの尺度で評価する。
  回答にはスコア、尺度の凡例、レベルごとの確率や confidence が含まれる。

確率は判断の不確実さを扱う材料であり、確実性や個別回答の正しさの保証ではない。
閾値、複数の回答の統合、追加確認や人へのエスカレーションはアプリケーション側で決める。
判定だけを根拠に、取り消せない操作などの副作用を自動実行しない。

## 利用方法

このスキルは概念の説明に限り、CLI の操作方法、環境設定、認証手順は扱わない。
`foundation decide` での利用手順と共通 spec は `decide-cli` スキルを参照する。

## 一次情報

- [System One](https://docs.typesafe.ai/concepts/system-one): モデルの役割、Jev、較正された確率と利用上の注意。
- [Primitives (Questions)](https://docs.typesafe.ai/primitives): Noul、Choice、Score の質問と回答の型。
- [System One API](https://docs.typesafe.ai/api/system-one): API の質問形式と回答スキーマ。
- [Models](https://docs.typesafe.ai/models): 現行モデルと別名。変更される情報はここで確認する。

公式情報にない挙動やモデル比較は推測せず、必要な一次情報を確認する。
