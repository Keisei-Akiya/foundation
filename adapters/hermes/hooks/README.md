# Hermes hooks (Rust)

Build the executable with `cargo build --release`. The executable reads one JSON object from stdin (maximum 1 MiB). Hook commands in `hooks.example.yaml` are argv: the binary path is one quoted argument and the Rust subcommand is the next argument. The examples are opt-in and are not installed into Hermes configuration automatically.

| Subcommand | Event and behavior | Output/limits |
|---|---|---|
| `preflight` | `pre_tool_call`; narrowly blocks literal `git commit --no-verify` and selected destructive PowerShell command forms. | Hermes blocking JSON `action` + `message`; lexical checks, not a shell parser. |
| `hookify` | `pre_tool_call`; reads flat scalar `hookify.*.local.md` rules from the project `.claude` directory, for `bash` and `file` events with block action. | Hermes blocking JSON `decision` + `reason`; malformed or unsupported rules fail open. |
| `config-protection` | `pre_tool_call`; blocks writes to existing protected configuration filenames. | Hermes blocking JSON `decision` + `reason`; new files are allowed. |
| `post-observer` | `post_tool_call`; reads supported source files and observes console calls plus selected design signals. | stderr warning only; read-only and cannot block. Hermes may capture stderr only in debug logs. |
| `lifecycle` | `post_tool_call` for the Skill tool; records minimal skill id/version/outcome. | Disabled by default; enable process environment `ECC_HERMES_SKILL_TRACK=1`; requires `HERMES_HOME`; writes JSONL with restrictive permissions. |

`check-console-log` is merged into `post-observer`; no compatibility wrapper is needed. `stop_lifecycle` was not connected in the shipped sample and was intentionally removed rather than ported. This is a purpose-limited adapter, not a complete port of all ECC hooks: see `INVENTORY.md` for the boundaries. Live Hermes callback integration has not been tested here.
