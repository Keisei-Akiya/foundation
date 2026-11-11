---
name: decide-cli
description: Use when calling a decision model with `foundation decide`. Covers spec format, providers, env vars and dry-run.
---

# decide-cli

`foundation decide` は、共通の質問 spec を OpenAI Decisions API か Microsoft Foundry（Jev）へ送る。
応答は JSON のまま標準出力へ出る。
背景知識は `decision-model-knowledge` スキルを参照する。

## 使い方

```
foundation decide --provider <openai|microsoft> [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>]
```

- `--spec` は省略または `-` で標準入力から読む。上限は4 MiB。
- `--model` は OpenAI では既定 `gpt-6-luna`。Microsoft ではデプロイ名で、省略時は `DEPLOYMENT_NAME` を使う。
- `--dry-run` は送信せず、送信先 URL とリクエスト本文を表示する。
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

## 環境変数

認証情報と接続設定は、まずプロセス環境変数から読み、値が設定されていない場合に限り Hermes の `.env` から補う。
`.env` は `$HERMES_HOME/.env`、`HERMES_HOME` が未設定なら `~/.hermes/.env` を読む。
dotenv 形式の `export`、コメント、単引用符・二重引用符に対応し、シェルとして実行しない。
CLI は値を表示しない。
会話・ログ・コミットにキーを書かない。

- openai: `OPENAI_API_KEY`（必須。OpenRouter の `OPENROUTER_API_KEY` は代用しない）、`OPENAI_BASE_URL`（任意）
- microsoft: `AZURE_ENDPOINT`（必須）、`AZURE_API_KEY` または `AZURE_ENTRA_TOKEN`、`DEPLOYMENT_NAME`

## 手順

1. spec を書く。
2. `--dry-run` で本文と送信先を確認する。
3. キーを設定して実行する。
4. 結果は判定の材料として使い、副作用のある操作は人が確認してから行う。
