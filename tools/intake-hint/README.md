# intake-hint 計測

human→AI の入口で、直前の AI 発言と最後のユーザー発言から独立した問い（フラグ）を `foundation decide` で得る設計（#136）の計測ハーネスである。
4フラグ版と5フラグ版を同じ発話セットで比べ、フラグごとの誤検出数と見逃し数を出す。
これは `foundation` プラグインの構成要素ではなく、配布物には含まれない。

## 構成

- `questions/`: 質問セット。`decide` の spec の `questions` 配列がそのまま入っている。
  - `baseline4`: `proposal`・`reply`・`new_task`・`discuss`。`new_task` は「新しい作業を依頼する内容を含むか」だけの旧説明。
  - `narrow4`: `baseline4` の `new_task` に「検討したい・考えたいだけ、質問したいだけ、の場合は含まない」を足した版。
  - `five`: `narrow4` に `standing_rule`（今後ずっと守るルールの指示を含むか）を足した版。
- `synthetic.jsonl`: 合成発話にラベルを付けたもの。
- `eval.py`: 検査・計測・再集計。標準ライブラリだけで動く。

## 発話セットの形式

1行1件の JSON である。

```json
{"id": "syn-001", "source": "synthetic", "previous_ai_message": "…", "user_message": "…",
 "labels": {"proposal": true, "reply": "approve", "new_task": false, "discuss": false, "standing_rule": false},
 "tags": ["reply_only"]}
```

- `source` は `synthetic` か `real`。集計は source ごとに別々に出る。
- `previous_ai_message` は直前の AI 発言で、なければ `null` にする。
- `reply` は `approve` / `revise` / `decline` / `none` のどれかである。
- `new_task` は、今動かす作業の依頼を含むときだけ true にする。単なる許可・承認、恒久ルールだけの発話は false にする。
- `standing_rule` は、今後ずっと守るルールや方針の指示を含むときに true にする。
- 発話セットには、恒久ルールの指示、提案形の依頼（「〜したら？」）、返事と並行作業指示の混合発話を含める。`tags` で見分けられる。

## 使い方

```
# 形式の検査（通信しない）
python3 -I tools/intake-hint/eval.py validate --dataset tools/intake-hint/synthetic.jsonl

# 送信先と本文の確認（送信しない）。実行前に必ずやる。
python3 -I tools/intake-hint/eval.py run --dataset tools/intake-hint/synthetic.jsonl --variants five --dry-run

# 計測（decide を発話数 × 版の数だけ呼ぶ）
python3 -I tools/intake-hint/eval.py run --dataset tools/intake-hint/synthetic.jsonl \
  --variants baseline4,narrow4,five --out /path/to/results.jsonl

# 保存済みの結果から再集計する（通信しない。しきい値を変えるときに使う）
python3 -I tools/intake-hint/eval.py report --dataset tools/intake-hint/synthetic.jsonl \
  --results /path/to/results.jsonl --threshold 0.6
```

- `--foundation` か環境変数 `FOUNDATION_BIN` で `foundation` バイナリを指定する。省略時は PATH 上の `foundation` を使う。
- 認証や接続先は `foundation decide` の規則に従う（`skills/decide-cli/SKILL.md`）。
- 述語は確信度がしきい値（既定 0.5）以上なら検出とみなす。0.3〜0.7 の件数は `uncertain` として別に数える。
- 出力は版 × source ごとの TP / FP / FN / TN と、誤検出・見逃しになった id の一覧である。`reply` は正解数と不正解数を出す。
- `--out` の結果ファイルには発話本文を書かない。id と回答だけである。

## 実発話を使う手順

実発話は会話履歴由来なので、このリポジトリには入れない。
`eval.py` は、`source` が `real` の発話をリポジトリの作業ツリーの中から読むと拒否する。

1. リポジトリの外の非公開の場所に、上と同じ形式で JSONL を作る。`source` は `real`、`id` は合成と重複させない（例: `real-001`）。
2. ラベルは人が付ける。ラベルを付けた人と、第三者ラベルの有無を記録しておく。
3. 合成と一緒に渡す。source ごとに別々に集計される。

```
python3 -I tools/intake-hint/eval.py run \
  --dataset tools/intake-hint/synthetic.jsonl --dataset /path/to/private/real.jsonl \
  --variants baseline4,narrow4,five --out /path/to/private/results.jsonl
```

実発話の標本を作り直すときは、抽出時の乱数の種と条件を残す。
過去の計測（60件）は種を残していないため、同じ標本の再現はできない。

## 限界

- 合成発話は書き手が作った例であり、実運用の分布を表さない。
- ラベルは主観で付けたもので、一般的な精度を示すものではない。
- 述語の判定はしきい値で二値化している。確信度が中間の発話の扱いは #139 で決める。
