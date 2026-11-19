# ADR 0002: foundation update をタグ時点の install.sh に委ねる

状態: 承認済み。

## 背景

Issue #126 で、導入済みの foundation を更新するコマンドを追加する。

これまで更新は、利用者が `curl … | sh` で `install.sh` を手で実行する方法しかなかった。

その結果、更新の要否に気づけず、導入済みの版が最新から2つ遅れる状況が起きた。

`install.sh` はすでにリリースの tar.gz の取得、SHA256 の検証、バイナリと共有ツリーの置換、プラグインの有効化を担っている。

## 決定

### 1. 更新処理を Rust に再実装せず、`install.sh` に一本化する

`foundation update` は、対象リリースのタグ時点の `install.sh` を取得して実行する。

取得、検証、置換の仕様が sh と Rust で二重になることを避けるためである。

実行中のバイナリ自身を置換する処理を Rust 側に持たずに済む。

`install.sh` が古い成果物の構成を想定しないよう、`main` ではなくタグ時点のものを使う。

`--hermes-home` の指定は、そのまま `install.sh` に渡す。

### 2. 最新版のタグは GitHub API で解決する

最新版のタグ名は、リリース API の JSON から読む。

リダイレクト先 URL の形式に依存するより壊れにくい。

未認証の呼び出し回数の制限は、この用途の頻度では問題にならない。

### 3. `install.sh` 自体のチェックサム検証は行わない

tar.gz は従来どおり SHA256 で検証する。

`install.sh` は、これまでの `curl | sh` と同じ信頼の範囲（HTTPS と GitHub）で取得する。

リリースへのチェックサム添付は、release workflow の変更を要するため、必要になってから検討する。

### 4. プラグインは常に有効にし、`foundation enable` を廃止する

foundation はプラグインとして使う前提であり、有効化は導入の一部である。

`foundation enable`、`install.sh --enable-hooks`、`--enable-plugin`、`hooks.example.yaml`、shell hooks の導入手順を削除する。

フックの実体（`preflight`、`hookify` など）は、プラグインが呼び出すため残す。

`config.yaml`、allowlist、gateway には触れない。

### 5. ロールバックを作らない

途中で失敗してバイナリと共有ツリーが食い違った場合は、`foundation version` で検知し、再度 `update` して直す。

## 結果

`update` の追加で増えるのは、引数の解析、版の比較、最新タグの解決、`install.sh` の取得と実行だけである。

置換と検証のロジックは `install.sh` に残る。

更新後は、gateway の再起動が必要なことを表示する。再起動は行わない。

## 戻すときの条件

`install.sh` を介さずに更新したい要件（sh が使えない環境、`install.sh` の検証が必要な運用など）が生じたら、この決定を見直す。

その場合は、新しい ADR で置き換える。
