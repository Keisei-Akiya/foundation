---
name: intake-hint
description: Use when classifying a human's entry utterance with `foundation intake-hint`; returns a hint or sends it back to the human.
---

# intake-hint

`foundation intake-hint` は、直前の AI 発言と人間の最後の発話を Jev（`foundation decide` と同じ経路）に渡し、発話の性質を表すフラグを返す。
自信が持てないときは、自動で行動を選ばずに人間へ戻す。

結果は参考情報である。承認や実行の制御には使わない。
spec の根拠は `docs/adr/0003-intake-hint.md` にある。

## 必要条件

- `foundation` 0.0.25 以上のバイナリが別途インストール済みであること。
- 認証と接続先は `foundation decide` の規則に従う（`skills/decide-cli/SKILL.md`）。
- このスキルはバイナリをインストールせず、設定やファイルを変更しない。

## 使い方

標準入力に JSON を1つ渡す。

```sh
echo '{"previous_ai_message": "対応方針を2案出しました。", "user_message": "A案でいい。ついでにテストも追加して"}' \
  | foundation intake-hint [--dry-run] [--timeout <seconds>]
```

- `user_message` は必須の空でない文字列。人間の最後の発話をそのまま渡す。
- `previous_ai_message` は文字列か `null`。直前の AI 発言がなければ `null`。
- `--dry-run` は、送信せずに、Jev へ送る本文を出力する。初めて使うときと、質問を変えたときは先に確認する。
- `--timeout` は Jev リクエストのタイムアウト秒数。既定値は `60`、指定可能範囲は `1`〜`600`。
- 入力が不正なとき（JSON でない、`user_message` が空、1 MiB 超）は、使い方を標準エラーに出して終了コード `2` で終わる。
- 判断できないときも終了コードは `0` で、結果の JSON が `route: "human"` を返す。

## 出力

### `route: "hint"`

```json
{
  "route": "hint",
  "flags": {"proposal": true, "new_task": true, "discuss": false, "standing_rule": false},
  "instruction": "new_task",
  "reply": "revise",
  "probabilities": {"proposal": 0.89, "new_task": 0.97, "discuss": 0.06, "standing_rule": 0.04}
}
```

- `flags` は、各フラグの Yes 確率が `0.5` 以上なら `true`。
  - `proposal`: 直前の AI 発言が承認や選択を求める提案か。
  - `new_task`: 今動かす作業の依頼を含むか。
  - `discuss`: 検討や相談だけか。
  - `standing_rule`: 今後ずっと守るルールや方針の指示を含むか。
- `instruction` は `"standing_rule"`、`"new_task"`、`null` のどれか。両方が立つときは `standing_rule` を優先する。
- `reply` は `approve` / `revise` / `decline` / `none` のどれか。取れなければ `null`。
- `probabilities` は、各フラグの Yes 確率。

### `route: "human"`（人間へ戻す）

```json
{"route": "human", "reason": "low_confidence", "probabilities": {"new_task": 0.5, "standing_rule": 0.02}}
{"route": "human", "reason": "undecidable"}
```

- `low_confidence`: `new_task` か `standing_rule` の確率が `0.35` 以上 `0.65` 以下（両端を含む）。
- `undecidable`: Jev に届かない、応答が JSON でない、フラグが欠けている、確率が範囲外。
- 人間に発話の意図を確認する。
  確認が済むまで、発話を自分で解釈して作業や承認を進めない（fail closed。`docs/adr/0003-intake-hint.md`）。
- `route: "human"` のときに `instruction` や `reply` はない。hint の値があるかのように扱わない。

## hint の使い方

- `instruction` が `"standing_rule"` なら、今回の作業とは別に、恒久ルールとして扱う候補を示す。
- `instruction` が `"new_task"` なら、新しい作業の依頼である可能性が高い。
- `instruction` が `null` で `reply` が `approve` / `decline` なら、直前の提案への返事である可能性が高い。
- `proposal`、`discuss`、`reply` は、ラベル定義とのずれが大きい。経路の判断には使わず、添える情報に留める。
- 帯の外でも誤りは残る。モデルが自信を持って外す例は、閾値では拾えない。

## 計測結果

数値は ADR 0003 から転記した。モデルは `jev-1.14.0`、計測は1回ずつ。同じ入力でも応答が揺れるので、差が1〜2件のものは効果として扱わない。
実発話と合成発話は分けて記録する。

### 実発話（60件、非公開）

- 標本は会話履歴から無作為抽出した。ラベルは AI エージェントが1人で付け、判断が割れる10件は `ambiguous`。
- `standing_rule`: 恒久ルール4件のうち3件で `p ≥ 0.5`。残り1件は `p = 0.41` で低信頼になる。ほかの56件の誤検出は0件。
- 指示（`new_task` または `standing_rule`）の見逃しは、4フラグ版で4件、5フラグ版で3件。誤検出は4件で変わらない。差は1件で、効果があったとは言えない。
- 低信頼の帯 `0.35〜0.65`: 人間へ戻すのは10件で、指示の誤り8件のうち4件を拾う。
- 帯の外に誤りが4件残る。
- `proposal` の誤検出20件、`discuss` の誤検出20件、`reply` の不正解26件。

### 合成発話（42件、`tools/intake-hint/synthetic.jsonl`）

- 書き手が作った例であり、実運用の分布を表さない。構成の比較と回帰の検出に使う。
- 低信頼の帯 `0.35〜0.65`: 人間へ戻すのは6件で、指示の誤り14件のうち5件を拾う。
- 帯の外に誤りが9件残る。
- 誤検出の件数は、再計測で11→12件と動いた。

### 限界

- 実発話の標本は小さい（60件）。恒久ルールは4件しかない。
- ラベルは主観で付けた。実発話は1人のラベルで、2人目の突き合わせはまだない。
- 閾値は、この標本での件数の比較から選んだ。一般的な精度を示すものではない。
- この計測は、Jev によって handoff の精度が改善したことを示すものではない。分類の振る舞いを確かめただけである。
- 標本が100件を超えるか、モデルの版が変わったときに、同じ手順で再計測する。手順は `tools/intake-hint/README.md`。実発話は非公開の場所に置き、このリポジトリには入れない。
