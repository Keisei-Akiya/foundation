---
name: decide-cli
description: Use when calling Jev or Microsoft-Decision through `foundation decide`. Covers spec format and dry-run.
---

# decide-cli

`foundation decide` は、共通の質問 spec を TypeSafe の System One API（Jev）または Azure AI Foundry の Microsoft-Decision に送り、応答 JSON を標準出力へ出す。

## 使い方

```
foundation decide [--provider typesafe|microsoft] [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>]
```

- `--provider` は省略でき、省略時は `typesafe` を使う。`microsoft` は下の「Microsoft」を参照。
- `--spec` は省略または `-` で標準入力から読む。上限は4 MiB。
- `--model` は provider ごとに扱いが違う。`typesafe` は省略時に `jev-latest` を使い、指定する場合も `jev-latest` のみ受け付ける。`microsoft` はデプロイ名で、省略時は `Microsoft-Decision-1.1` を使う。
- `--dry-run` は送信せず、送信先 URL とリクエスト本文を表示する。URL の許可規則は dry-run にも適用する。
- HTTP リダイレクトは追跡しない。
- 終了コードは、成功が 0、引数・spec・環境変数の誤りが 2、通信や HTTP エラーが 1 である。

## Microsoft

`--provider microsoft` は Foundry の `<endpoint>/providers/microsoft/v1/systemone` に、Jev と同じ形の本文を送る。
認証は Entra ID の Bearer トークンで、API キーは使わない。

- `MS_DECISION_ENDPOINT`（必須）は Foundry リソースの URL。https のみで、資格情報・パス・クエリ・フラグメントは拒否する。`~/.hermes/.env` からも読む。
- トークンは `AZURE_ENTRA_TOKEN` があればそれを使い、無ければ `az account get-access-token --resource https://cognitiveservices.azure.com` を実行して得る。トークンは `.env` から読まない。
- 呼び出す ID に、Foundry リソースへのロール割り当てが必要になる。403 のときは割り当てを確認する。
- `--dry-run` はトークンを取得せず、URL と本文だけを表示する。

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
