---
name: fp-rust-check
description: Use when checking Rust code against fp-principles. clippy deny set plus manual review items.
---

# fp-rust-check

Rust のコードが `fp-principles` を守っているかを確認する手順をまとめる。
原則と判断は `fp-principles` が正本である。
確認は、lint で機械的に拾う部分と、レビューで人かエージェントが見る部分に分ける。

## 機械チェック

`clippy.toml`（このスキルに同梱）を crate の直下に置き、純粋層のモジュールにだけ `deny` を付ける。
副作用層には `allow` を付ける。

```rust
#[allow(clippy::disallowed_methods, clippy::disallowed_macros)]
pub mod effects { /* I/O はここに置く */ }

pub mod pure {
    #![deny(
        clippy::disallowed_methods, clippy::disallowed_macros, clippy::disallowed_types,
        clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing,
        clippy::wildcard_enum_match_arm, clippy::let_underscore_must_use,
        clippy::print_stdout, clippy::dbg_macro, clippy::todo, clippy::unimplemented,
        clippy::must_use_candidate
    )]
}
```

- `disallowed_*` は `clippy.toml` の一覧に載せた呼び出しと型を禁止する。
  この lint は crate 全体で warn になる。
  `-D warnings` で clippy を実行する場合は、副作用層に `allow` を付けないと失敗する。
- `wildcard_enum_match_arm` は、`_ =>` で未来の variant を黙って受けることを防ぐ。
- `let_underscore_must_use` は、`let _ = ...` で `Result` を握りつぶすことを防ぐ。
- `must_use_candidate` は、純粋な関数の戻り値を捨てる誤用を防ぐ。
- 既存の `-D warnings` の clippy 実行と同じコマンドで確認できる。

## レビューで見るもの

次は lint で書けないので、人かエージェントが見る。

- trait の裏に I/O が隠れていて、純粋層に漏れていないか。
- newtype にすべき値が、生の `String` や `u64` のままになっていないか。
- 状態遷移が `enum` か typestate で表されているか。
- `Result` を握りつぶしていないか（`.ok()` で捨てるなど）。

## 実測（clippy 0.1.97、使い捨ての crate）

- `fs::read_to_string`、`fs::write`、`Instant::now`、`RefCell`、`println!` は、`deny` を付けた純粋層で error になった。
- 同じ呼び出しは、`allow` を付けた副作用層では `-D warnings` でも通った。
- `unwrap`、配列の添字、`_ =>`、`let _ =`、`todo!`、`#[must_use]` の付け忘れも、純粋層で error になった。
- `needless_pass_by_ref_mut` は nursery の lint で、既定では発火しなかった。
  `&mut Vec` の引数は、代わりに `ptr_arg`（warn）が指摘した。
- `fn_params_excessive_bools` と `mut_mut` は、今回の入力では発火を確認していないため、載せていない。
