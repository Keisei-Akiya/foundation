---
name: decision-review
description: Use when reviewing changes with reusable decision-model question sets.
---

# decision-review

差分と判断に必要なファイル本文を `foundation decide` に渡し、開発時の確認材料となる質問ごとの判断確率を得る。
結果は合否や正しさの証明ではない。
判断モデルの概念は `decision-model-knowledge`、CLI の仕様は `decide-cli` を参照する。

## 質問セット

`questions.json` の `dev` 質問セットは、言語に依存しない関数型設計の観点を確認する。
`fp-principles` の一部の原則を質問にしたものであり、関数型設計全体を網羅しない。
各質問は記載された命題を尋ねるため、命題が当てはまることだけで常に原則違反だとは限らない。

## 実行

1. 差分と判断に必要なファイル本文を入力ファイルにまとめる。
   複数ファイル間の関係を見る場合は、差分だけでなく比較対象の本文も含める。
2. インストール済みスキルの `run.py` を実行し、provider を明示する。
   `python3 -I "${HERMES_HOME:-$HOME/.hermes}/foundation/skills/decision-review/run.py" <typesafe|microsoft|openai|openai-codex> <input-file>`
   `foundation` バイナリは `PATH` から解決し、質問セットを使って `foundation decide --provider <provider> --spec -` を呼ぶ。
3. スクリプトは質問ごとの生の確率を全て出力し、確率による足切りや低い値の除外をしない。
   出力値は `foundation decide` の provider 応答に含まれた確率であり、正しさや誤りを証明しない。
4. コマンドが非ゼロ終了した場合、不正な応答を受けた場合、質問への回答や確率が欠けた場合は非ゼロ終了し、「未確認」と報告する。
   エラーや部分回答を成功した低確率の結果として扱わない。

判断結果だけを根拠に commit や push を止めない。
