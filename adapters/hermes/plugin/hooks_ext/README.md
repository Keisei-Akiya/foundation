# hooks_ext

1 hook 1 ファイルで置く。`_` で始まるファイルと `.md` は読み込まれない。
各ファイルは `register(ctx, core)` を持つ。
`core.run_subcommand(name, payload, strict=False, timeout=5)` は `foundation <name>` を呼ぶ。
`core.payload(event, kwargs)` は shell hook と同じ stdin JSON を作る。
`core.hermes_home()` は `HERMES_HOME` を返す。
読み込みに失敗した拡張はログに残して読み飛ばす。他の hook は止まらない。
