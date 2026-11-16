---
name: wiki-search
description: Use when answering from a local LLM wiki; rank candidate pages with `wiki_search.py` before reading their contents.
---

# wiki-search

`wiki_search.py` は、会話の文脈を補った質問で Wiki のページ候補を選び、関連度順の一覧を返す。回答を作る AI は一覧にあるページ本文を読み、質問への回答に使う。

## 使い方

```sh
python3 <foundation>/skills/wiki-search/scripts/wiki_search.py \
  --wiki <wiki-root> [--threshold <0..1>] [--max-results <count>] \
  [--domain-threshold <0..1>] [--max-domains <count>] [--log <jsonl-path>] \
  "<文脈を補った質問>"
```

- `--wiki` は必須。Wiki のルートディレクトリを指定する。
- `question` は必須の位置引数。会話の文脈を含め、何を知りたいかを一文で書く。
- `--threshold` はページ関連スコア `P(low)+P(high)` の下限。既定値は `0.2`。
- `--max-results` は一覧の最大ページ数。既定値は `10`。
- `--domain-threshold` はドメイン選択に使う `P(yes)` の下限。既定値は `0.2`。
- `--max-domains` は選択するドメインの最大数。既定値は `5`。
- `--log` は JSONL ログの追記先。既定値は `~/.local/state/foundation/wiki-search.jsonl`。
- スコア閾値は `0`〜`1`、最大件数と最大ドメイン数は `1` 以上。

例:

```sh
python3 <foundation>/skills/wiki-search/scripts/wiki_search.py \
  --wiki ~/wiki "DecisionRAG の検索設計で、候補ページの選び方を知りたい"
```

`SCHEMA.md` とドメインタグがある場合、まず各ドメインの `P(yes)` でドメインを選び、選ばれたドメインのページと `domains:` の無いページだけを採点候補にする。ドメイン閾値を超えるものが無い場合は最も確率の高いドメインを選ぶ。`SCHEMA.md` が無い、またはドメインタグが無い場合は全ページが候補になる。ページ候補のスコアがすべて `--threshold` 未満なら、最高スコアのページを1件返す。

## 出力の読み方

標準出力は1ページ1行で、タブ区切りの `スコア<TAB>相対パス<TAB>タイトル`。スコアの高い順に並ぶ。表示されたパスのページ本文を読み、質問に関係する内容を回答に反映する。

`--log` の JSONL は呼び出しごとに1行で、各レコードに次のフィールドがある。

- `timestamp`: UTC の記録時刻。
- `question`: 採点に使った質問。
- `domain_probabilities`: ドメイン名から `P(yes)` への対応。ドメイン選択を行わない場合は空のオブジェクト。
- `page_scores`: Wiki 内の相対パスからページスコアへの対応。**採点候補外のページは `null`**。
- `returned_pages`: 標準出力に返したページの相対パス。
- `jev_usage`: Jev 呼び出しの使用量オブジェクトの配列。

`page_scores` の `null` は未採点を意味し、スコア `0` ではない。
