# リポジトリの前提

このリポジトリ全体が一つのHermesプラグイン `foundation` である。
名前、バージョン、配布の単位はリポジトリ単位であり、バイナリ名もプラグイン名と同じ `foundation` とする。
`adapters/`、`skills/`、`docs/`、`src/`（バイナリ）はプラグインの構成要素であり、個別の製品や配布物ではない。
構成要素に独立した名前やバージョンを付けない。
新しい名前を付ける前に、それがプラグイン全体の名前か構成要素の名前かを区別する。
配布物は `<HERMES_HOME>/bin/foundation` と `<HERMES_HOME>/foundation/` に、常にセットで配置する。
Hermesのネイティブプラグイン定義は `adapters/hermes/plugin/`（`plugin.yaml` と `__init__.py`）にある。
`install.sh` は `<HERMES_HOME>/plugins/foundation` から `<HERMES_HOME>/foundation/adapters/hermes/plugin` へのシンボリックリンクを置き、Hermes CLIで常にプラグインを有効化する。
プラグインは `register(ctx)` でフックを登録し、各コールバックが `foundation` バイナリをsubprocessで呼ぶ。
プラグインを無効にする場合は `hermes plugins disable foundation` を実行する。

# 文章ルール

日本語の説明文は常体で書く。
体言止めは使わず、文を言い切る。
句点「。」の後は必ず改行する。
見出し、コード、ファイル名はこの文章ルールの対象外とする。
