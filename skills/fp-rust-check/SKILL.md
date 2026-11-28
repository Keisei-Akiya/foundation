---
name: fp-rust-check
description: Use when checking Rust code against fp-principles. clippy deny set plus Jev judgments.
---

# fp-rust-check

Rust のコードが `fp-principles` を守っているかを、clippy と判断モデル（Jev）の2段で確認する。
原則は `fp-principles`、書き方は `fp-rust` が正本である。
判断モデルの概念は `decision-model-knowledge`、CLI は `decide-cli` を参照する。

## 分担

- 機械で言い切れるもの（`unwrap`、時刻の取得、`_ =>`）は、clippy の `deny` が拾う。
- 読まないと分からないもの（生の ID、`bool` で表した状態、握りつぶしたエラー）は、Jev が判断する。
- 判断を受けて何を報告するかは `review.py` の判定表が決める。
  Jev に「直すべきか」は聞かない。

## 機械チェック

`clippy.toml` を crate の直下に置き、純粋層のモジュールにだけ `deny` を付ける。
`deny` の組、`allow` の付け方、実測は [references/clippy.md](references/clippy.md) を参照する。

## 判断モデル

質問は `questions.json` にある。
1問が1つの判断で、すべて Noul（文が真である確率）である。
`state` は `{"files": {ファイル名: ソース}}` で、判断に要るファイルだけを渡す。

1. `python3 -I "${HERMES_HOME:-$HOME/.hermes}/foundation/skills/fp-rust-check/review.py" spec a.rs b.rs > spec.json`
2. `foundation decide --dry-run --spec spec.json` で本文を確認する。
3. `foundation decide --spec spec.json > response.json` を実行する。
   失敗（終了コードが 0 以外）は「該当なし」ではなく、エラーとして扱う。
4. `python3 -I .../review.py judge response.json` で判定表を適用する。

## 判定表

| Noul | 判定 | 扱い |
| --- | --- | --- |
| 0.7 以上 | yes | 指摘候補。該当箇所を読んで確認する。 |
| 0.3 超 0.7 未満 | unsettled | `state` だけでは決まらない。人が確認する。 |
| 0.3 以下 | no | 出さない。ただし問題がないとは言い切らない。 |

Noul は位置を返さない。
yes と unsettled は、人かエージェントがコードを読んで、該当箇所を探す。
判定を根拠に、commit や push を止めない。

## 質問を変えるとき

- 1問に1つの判断だけを書く。「または」でつなぐなら、2問に分ける。
- 外れたケースのために、文言へ節を足さない。直すのは、`state` か、質問の分割か、`review.py` である。
- 変えたら、調整に使っていないケースで再測定する。手順と結果は [references/measurement.md](references/measurement.md) を参照する。
