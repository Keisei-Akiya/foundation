# skills

スキルに関するファイルを配置する。
これは `foundation` プラグインの構成要素であり、独立した製品ではない。

## 判断モデル

- `decision-model-knowledge/`: 判断モデル、Jev、質問と回答の型の概念。
- `decide-cli/`: `foundation decide` の使い方。

## 検索

- `wiki-search/`: `foundation wiki-search` の使い方。バイナリは別途インストールして利用する。

## レビューと入口の補助

- `decision-review/`: 文書・テスト・言語共通の関数型設計について、`foundation decide` で判断材料を得る質問セット、spec 生成、過去の計測結果。
- `intake-hint/`: `foundation intake-hint` の使い方と、人間へ戻す扱い、計測結果。

## 関数型プログラミング

- `fp-principles/`: 関数型設計の原則と、迷ったときの判断。この分野の正本であり、他のスキルはこれを参照する。

原則（根）の下に、書き方とチェックの仕方（中間）があり、その下に言語別のスキル（葉）がある。

- 書き方
  - `fp-rust/`: 原則を Rust の `enum`、newtype、`Result`、typestate に当てはめる書き方。
- チェックの仕方
  - `decision-review/`: 言語共通の関数型設計に関する絞った質問を含む。
    決定的な機械チェックはプロジェクトの hook が担い、複雑な設計判断は LLM が確認する。
