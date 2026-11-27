# インストールと更新

`foundation` の導入、バージョン確認、更新の手順を記す。

## インストール

Linux x86_64 のみ対応する。
GitHub Releases のビルド済み成果物を取得し、SHA256を検証してから `<HERMES_HOME>`（既定 `~/.hermes`）に配置する。

```sh
curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh
```

バージョンを固定する場合は `sh -s -- --version X.Y.Z` を付ける。`X.Y.Z` は説明用のプレースホルダーなので、実際に公開されているリリースのバージョンに置き換える。
配置先は次のとおり。

- `<HERMES_HOME>/bin/foundation`
- `<HERMES_HOME>/foundation/`（adapters、skills、docs、VERSION）

インストーラーはHermes CLIを使ってネイティブpluginを有効にし、`skills.external_dirs` に `<HERMES_HOME>/foundation/skills` を登録する。
既存の外部skillディレクトリは保持し、同じディレクトリを重複登録しない。
この登録により、foundationのskillをHermesのskill検索対象に含める。
それ以外の設定とhook許可設定は変更せず、起動中のgatewayも再起動しない。
反映にはgatewayの再起動が必要である。
Hermes CLIが見つからない場合は、配置を完了した後にエラーで停止する。
プラグインを無効にする場合は `hermes plugins disable foundation` を実行する。

## バージョン確認と更新

導入済みのバイナリと共有ツリーの版は、`foundation version` で確認する。このコマンドはネットワークに接続しない。共有ツリーの `VERSION` ファイルが無い場合は、その版を `unavailable (VERSION file not found)` と表示する。

```sh
foundation version
```

最新版への更新の要否は `foundation update --check` で確認する。最新の版、バイナリの版、共有ツリーの版、および `Update available` または `Already up to date` が表示される。更新があっても終了コードは 0 であり、このコマンドはファイルを変更しない。

```sh
foundation update --check
```

最新版へ更新するには、次を実行する。

```sh
foundation update
```

すでに最新で、バイナリと共有ツリーの版も一致している場合は `最新です (version X.Y.Z)` と表示して何も変更しない。更新が成功すると `更新が完了しました。gateway の再起動が必要です。` と表示する。更新コマンドは gateway を再起動しないため、更新を反映するには利用者が gateway を再起動する必要がある。

特定の版へ更新するには `--version X.Y.Z` を指定する。指定した版が存在しない場合はインストーラーの取得が失敗し、エラーで終了する。既存のバイナリや共有ツリーはその場合変更されない。新しい版から以前の版へ戻す（ダウングレードする）場合も同じ指定を使う。

```sh
foundation update --version X.Y.Z
```

`version` と `update` の対象 Hermes home は、`--hermes-home DIR`、環境変数 `HERMES_HOME`、既定の `~/.hermes` の順で決まる。別のプロファイルを対象にする場合は、次のように両コマンドで指定する。

```sh
foundation version --hermes-home /path/to/hermes-home
foundation update --check --hermes-home /path/to/hermes-home
foundation update --hermes-home /path/to/hermes-home
```

リリース後に実物で一度だけ確認する手順は、[実リリース向け手動スモークチェックリスト](update-release-smoke-checklist.md) を参照する。

Rustがある環境では次でも導入できる。この方式はバイナリのみを配置し、adapters等は含まない。

```sh
cargo install --git https://github.com/Keisei-Akiya/foundation --locked --bin foundation
```
