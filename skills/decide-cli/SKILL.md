---
name: decide-cli
description: Use when calling Jev or Microsoft-Decision through `foundation decide`. Covers spec format and dry-run.
---

# decide-cli

`foundation decide` は、共通の質問 spec を TypeSafe の System One API（Jev）、Azure AI Foundry の Microsoft-Decision、または OpenAI Decisions に送り、応答 JSON を標準出力へ出す。

## 使い方

```
foundation decide [--provider typesafe|microsoft|openai|openai-codex] [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>]
```

- `--provider` は省略でき、省略時は `FOUNDATION_DECIDE_PROVIDER` を使い、未設定または空なら `openai-codex` を使う。Jev を使う環境は `FOUNDATION_DECIDE_PROVIDER=typesafe`、Microsoft のみを使う環境は `FOUNDATION_DECIDE_PROVIDER=microsoft` を設定する。`microsoft` は下の「Microsoft」、`openai` と `openai-codex` は下の「OpenAI」を参照。
- `--spec` は省略または `-` で標準入力から読む。上限は4 MiB。
- `--model` は provider ごとに扱いが違う。`typesafe` は省略時に `jev-latest` を使い、指定する場合も `jev-latest` のみ受け付ける。`microsoft` はデプロイ名で、省略時は `Microsoft-Decision-1.1` を使う。OpenAI の両 provider は `gpt-6-luna` のみ。
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

## OpenAI

`openai` は `/v1/decisions` に `OPENAI_API_KEY_DECISIONS` の Bearer 認証で送る。キーは環境変数のみから読み、`.env` からは読まない。ユーザーは Bitwarden Secrets Manager から注入する（例: `bws run -- foundation decide --provider openai ...`）。エージェントは `bws` を実行しない。

`openai-codex` は実験的で、`$CODEX_HOME/auth.json`（未設定なら `~/.codex/auth.json`）の `tokens.access_token` を使う。Codex token 利用時の課金先と、Codex 外での利用が OpenAI の規約上許可されるかは未検証。iii-hq/workers PR #1800 は 2026-11-19 の live e2e で `/v1/decisions` が Codex token に応答したと報告し、先行する rho PR #1433 は GA 前に 401 を観測した。

`intake-hint` と `wiki-search` も同じ `FOUNDATION_DECIDE_PROVIDER` を使う（未設定時は `openai-codex`）。Jev を使う場合は `FOUNDATION_DECIDE_PROVIDER=typesafe` を設定する。これら内部経路では OpenAI の `answers` 配列を Jev 形に正規化するが、`foundation decide` の標準出力は生の provider 応答のまま。

料金は未キャッシュ入力 100 万トークンあたり $0.10、出力は無料。

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
