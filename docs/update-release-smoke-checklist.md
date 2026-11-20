# 実リリース向け手動スモークチェックリスト

コマンドの利用方法は [README の「バージョン確認と更新」](../README.md#バージョン確認と更新) を参照する。

リリース後に、配布された `foundation` の実物を使って一度だけ実行する手動確認である。自動テストやモックでは代替しない。実リリースへの更新は実環境を変更するため、実施者と対象 Hermes home を事前に確認し、利用者の承認を得てから行う。

確認対象のバージョンを `X.Y.Z` とする。以下のコマンドは同じインストール済みバイナリと Hermes home に対して、記載順に一度ずつ実行する。`--hermes-home` を省く場合は `HERMES_HOME`、次に `~/.hermes` が対象になる。意図した環境か事前に確認する。

## 実施前

- [ ] 公開済み GitHub Release とタグがあり、タグが `vX.Y.Z` である。
- [ ] 対象ホストが Linux x86_64 で、更新対象の Hermes home を特定した。
- [ ] Hermes gateway を再起動できる時間帯であることを確認した。更新コマンド自体は gateway を再起動しない。
- [ ] 実施者の承認を得た。コマンドの出力と実施日時を #126 に記録する準備ができている。

## 手順

### 1. インストール済み版を確認する

```sh
foundation version
```

- [ ] 終了コードが 0 である。
- [ ] `Binary version: A.B.C` と `Shared tree version: D.E.F` が出力され、値を記録した。共有ツリーの版が `unavailable (VERSION file not found)` の場合は正常なリリース状態とみなさず、以降を進めず切り分ける。
- [ ] バイナリと共有ツリーの版が一致する。異なる場合は `Warning: binary version ... does not match shared tree version ...` が標準エラーに出るため、その状態を記録して切り分ける。

このコマンドはネットワークを使わない。バイナリの版は実行中のバイナリのビルド版、共有ツリーの版は対象 Hermes home の `foundation/VERSION` の値である。

### 2. リリースを確認する

```sh
foundation update --check
```

- [ ] 終了コードが 0 である。
- [ ] 標準出力に `Latest version: X.Y.Z`、`Binary version: A.B.C`、`Shared tree version: D.E.F` が出る。`Latest version` が確認対象の公開リリースと一致する。
- [ ] 対象バージョンより古い場合は `Update available`、すでに対象バージョン以上の場合は `Already up to date` が出る。
- [ ] コマンドの前後でバイナリと共有ツリーの状態が変わっていない。

更新がある場合でも終了コードは 0 である。この手順ではまだ更新しない。

### 3. 更新を実行する

```sh
foundation update
```

- [ ] 更新が必要だった場合は終了コードが 0 で、インストーラーが `Installed foundation X.Y.Z` と `The native plugin is enabled.` を出力し、最後に `更新が完了しました。gateway の再起動が必要です。` と表示する。
- [ ] 更新が不要でバイナリと共有ツリーの版も一致していた場合は、終了コードが 0 で `最新です (version X.Y.Z)` と表示し、再配置を行わない。
- [ ] 更新が行われた場合、運用手順に従って gateway を再起動し、再起動後にプラグインが反映されたことを確認する。`foundation update` 自体が gateway を再起動しないことを確認する。

## 失敗時の切り分け

- `version` が非 0 で終了する、共有ツリーの版が読めない、または版が食い違う場合は、対象の `--hermes-home` / `HERMES_HOME` と `foundation/VERSION` を確認する。食い違いのままでは成功と判定せず、状態を記録して運用担当に引き継ぐ。
- `update --check` が `could not determine latest version` で失敗する場合は、ネットワーク、GitHub Releases API への接続、および公開リリースを確認する。`Latest version` が期待したタグと異なる場合も先へ進まず、公開状態を調査する。
- `update` が `installer download failed`、`HTTP 404`、`SHA256 verification failed`、`hermes CLI not found`、または `enabling the native plugin failed` で失敗する場合は、出力をそのまま保存し、対象リリースの成果物、実行環境のネットワークと依存コマンド、Hermes CLI の利用可否を確認する。失敗後は `foundation version` でバイナリと共有ツリーの状態を確認する。
- 更新が完了したのに変更が反映されない場合は gateway の再起動状態を確認する。更新コマンドは gateway を再起動しない。
- 原因が解消する前に、このチェックリストの実リリース手順を繰り返さない。確認結果と残った問題を #126 に記録する。

## #126 への記録欄

以下は記録用フォーマットであり、実施記録ではない。実施後に #126 へ転記する。

```text
実施日時（JST）:
実施者:
対象ホスト / Hermes home:
公開リリース（タグ）:

1. foundation version
終了コード:
Binary version:
Shared tree version:
標準エラー / 警告:

2. foundation update --check
終了コード:
Latest version:
判定（Update available / Already up to date）:
状態変更なし（はい / いいえ）:

3. foundation update
終了コード:
結果（更新完了 / 最新のため変更なし / 失敗）:
出力（成功・エラーを原文で記録）:
更新後の version 確認:
gateway 再起動・反映確認（実施 / 不要 / 未実施）:

総合判定（成功 / 失敗）:
切り分け・残課題:
```
