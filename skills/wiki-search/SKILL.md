---
name: wiki-search
description: Use when answering from a local LLM wiki; find candidate pages with `foundation wiki-search` before reading their contents.
---

# wiki-search

`foundation wiki-search` は、会話の文脈を補った質問から Wiki の候補ページを選び、関連度順の一覧を返す。回答を作る AI は一覧のページ本文を読み、質問への回答に使う。

## 必要条件

- `foundation` **TODO(release): minimum foundation version** 以上のバイナリが別途インストール済みであること。
- Wiki のルートディレクトリがローカルで読み取り可能であること。
- このスキルはバイナリをインストールせず、`config.yaml` や Wiki の内容を変更しない。

## 使い方

```sh
foundation wiki-search --wiki <wiki-root> \
  [--domain-threshold <0..1>] [--domain-max <count>] \
  [--threshold <0..1>] [--max-results <count>] \
  [--timeout <seconds>] [--log-path <jsonl-file>] \
  "<文脈を補った質問>"
```

例:

```sh
foundation wiki-search --wiki ~/wiki \
  "先ほどの決定を踏まえ、DecisionRAG の検索設計で候補ページをどう選ぶか知りたい"
```

- `--wiki` は必須。Wiki のルートディレクトリを指定する。
- 質問は必須の位置引数。会話の文脈を補い、いま知りたいことを具体的な文にして渡す。
- `--domain-threshold` はドメイン採点の `P(yes)` 下限。既定値は `0.1`。
- `--domain-max` は残すドメインの最大数。既定値は `5`。
- `--threshold` はページ関連スコア `P(low) + P(high)` の下限。既定値は `0.2`。
- `--max-results` は返すページの最大件数。既定値は `10`。
- `--timeout` は Jev リクエストのタイムアウト秒数。既定値は `60`、指定可能範囲は `1`〜`600`。
- `--log-path` は JSONL ログの追記先。既定値は `$XDG_STATE_HOME/foundation/wiki-search.jsonl`。`XDG_STATE_HOME` が未設定の場合は `~/.local/state/foundation/wiki-search.jsonl`。
- 閾値は `0`〜`1`、最大数・最大件数は `1` 以上。

## 検索と出力

検索はドメイン、ページの2段階で行う。ドメイン名と説明は Wiki の `SCHEMA.md` から読み、ドメインごとに独立して採点する。閾値以上のドメインを最大 `--domain-max` 件残し、該当がなければ確率が最も高い1ドメインを残す。`domains:` の無いページは常にページ候補に含める。

残したドメインのページを採点し、複数ドメインに属するページは1回だけ扱う。ページがすべて `--threshold` 未満なら最高スコアの1件を返す。結果はスコア降順の1ページ1行で、タブ区切りの `スコア<TAB>相対パス<TAB>タイトル`。表示されたパスの本文を読み、質問に関係する情報を回答に反映する。

## ログ

呼び出しごとに JSONL を1行追記する。既定の保存先は上記のとおりで、`--log-path` で変更できる。各レコードには UTC の `timestamp`、質問の `input`、全ドメインの確率 `domain_probabilities`、全ページの `page_scores`、返したページ `returned_pages`、Jev の使用量 `usage` が含まれる。ドメインで候補外となり採点されなかったページのスコアは `null`。ページ本文は Jev に送られず、ログにも記録されない。
