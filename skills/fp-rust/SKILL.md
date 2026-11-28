---
name: fp-rust
description: Use when writing Rust in a functional style. Maps fp-principles to enum, newtype, Result.
---

# fp-rust

Rust で関数型の考え方を使って実装するときの書き方をまとめる。
原則と判断は `fp-principles` が正本である。
このスキルは、その原則を Rust の型、エラー処理、副作用の扱いに当てはめる部分だけを書く。
判断モデルによる言語共通の確認は `decision-review` の `dev` 質問セットを使う。
決定的な機械チェックはプロジェクトの hook を使う。
複雑な設計判断は、必要な文脈を渡して LLM に確認させる。

## Rust での当てはめ

番号は `fp-principles` の原則の番号である。

1. （原則1）純粋層と副作用層は、モジュールで分ける。
   時刻や設定は、純粋層の関数の引数で受け取る。
2. （原則2）取りうる状態は `enum` で表し、ペイロードは各 variant に持たせる。
   `bool` や `Option` の組み合わせで状態を表さない。
3. （原則2）意味のある値は newtype にする。
   private フィールドの tuple struct にし、`new` や `TryFrom` で検証して `Result` を返す。
   ID やメールアドレスを生の `String` のまま関数間で渡さない。
4. （原則3）失敗は `Result` で返し、`?` と `map`、`and_then` で連鎖させる。
   複数の入力を検証するときは、最初の失敗で止めず `Vec<Error>` に集める。
5. （原則2）状態遷移に順序がある場合は、遷移ごとに型を分ける（typestate）。
   `Order<Draft>` から `Order<Validated>` へ、検証関数だけが変換する。
6. （原則4）`match` は variant を全部書く。
   `_ =>` で残りを受けない。
7. （原則5）変換は iterator の `map` と `filter`、集計は `fold` で書く。
8. （原則1）外部との境界は trait（port）にし、実装（adapter）を副作用層に置く。
   純粋層のテストは、副作用層を使わずに値だけで書く。
