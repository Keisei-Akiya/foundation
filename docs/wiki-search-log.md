# wiki-search のログ

`foundation wiki-search` は、実行のたびに1件のJSONLレコードを `$XDG_STATE_HOME/foundation/wiki-search.jsonl` に追記する。
`XDG_STATE_HOME` が未設定の場合は `~/.local/state/foundation/wiki-search.jsonl` を使う。
保存先は `--log-path <file>` で変更できる。
レコードには入力テキスト、ドメインとページのスコア、返したページのパス、Jevの使用量を含む。
ページ本文は記録しない。
