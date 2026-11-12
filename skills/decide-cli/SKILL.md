---
name: decide-cli
description: Use when calling Jev through `foundation decide`. Covers spec format and dry-run.
---

# decide-cli

`foundation decide` は、共通の質問 spec を TypeSafe の System One API に送り、応答 JSON を標準出力へ出す。

## 使い方

```
foundation decide [--provider typesafe] [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>]
```

- `--provider` は省略でき、省略時は `typesafe` を使う。
- `--spec` は省略または `-` で標準入力から読む。上限は4 MiB。
- `--model` は省略時に `jev-latest` を使う。指定する場合も `jev-latest` のみ受け付ける。
- `--dry-run` は送信せず、送信先 URL とリクエスト本文を表示する。URL の許可規則は dry-run にも適用する。
- `TYPESAFE_BASE_URL` は既定の `https://api.typesafe.ai` またはテスト用ループバック URL のみ許可する。認証情報、パス、クエリ、フラグメントを含めてはならない。
- HTTP リダイレクトは追跡しない。
- 認証には `TYPESAFE_API_KEY` を使う。CLI はプロセス環境変数から読み、未設定の場合は `$HERMES_HOME/.env`（未設定時は `~/.hermes/.env`）から自動で補う。キー値を表示しない。
- 終了コードは、成功が 0、引数・spec・環境変数の誤りが 2、通信や HTTP エラーが 1 である。

## spec

```json
{
  "input": "問い合わせ本文、または JSON",
  "questions": [
    {"name": "dup", "type": "predicate", "instructions": "二重請求か"},
    {"name": "dept", "type": "choice", "instructions": "担当を選ぶ",
     "choices": [{"value": "billing", "description": "支払い"},
                 {"value": "tech", "description": "技術"}]},
    {"name": "sev", "type": "score", "instructions": "緊急度",
     "levels": [{"label": "low"}, {"label": "high"}]}
  ]
}
```

`predicate` は Noul、`choice` は Choice、`score` は Score に変換する。
choice の値ごとに説明を指定し、score の levels は順序付きで指定する。

## 手順

1. spec を書く。
2. `--dry-run` で本文と送信先を確認する。
3. 実行する。
