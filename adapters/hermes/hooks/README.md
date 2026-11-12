# Hermes hooks (Rust)

Build the executable with `cargo build --release`. The executable reads one JSON object from stdin (maximum 1 MiB). Hook commands in `hooks.example.yaml` are argv: the binary path is one quoted argument and the Rust subcommand is the next argument. The examples are opt-in and are not installed into Hermes configuration automatically.

| Subcommand | Event and behavior | Output/limits |
|---|---|---|
| `preflight` | `pre_tool_call`; runs for Bash/terminal and PowerShell tool names, narrowly blocking literal `git commit --no-verify` and selected destructive PowerShell forms. | Hermes blocking JSON `action` + `message`; lexical checks, not a shell parser. |
| `hookify` | `pre_tool_call`; runs for Bash/terminal and recognized file tools, reading flat scalar `hookify.*.local.md` rules from the project `.claude` directory for `bash` and `file` events with block action. | Hermes blocking JSON `decision` + `reason`; malformed or unsupported rules fail open. |
| `config-protection` | `pre_tool_call`; blocks writes to existing protected configuration filenames. | Hermes blocking JSON `decision` + `reason`; new files are allowed. Edit failures fail closed. |
| `post-observer` | `post_tool_call`; reads supported source files and observes console calls plus selected design signals. | stderr warning only; read-only and cannot block. Hermes may capture stderr only in debug logs. |
| `lifecycle` | `post_tool_call` for the Skill tool; records minimal skill id/version/outcome. | Disabled by default; enable process environment `ECC_HERMES_SKILL_TRACK=1`; requires `HERMES_HOME`; writes JSONL with restrictive permissions. |

The native callback runs `preflight` first for shell and PowerShell tool names, then runs `hookify` for shell and supported file-tool names. Any block short-circuits later checks. `config-protection` runs last for edit tools and fails closed if its binary invocation fails; `preflight` and `hookify` remain fail-open on execution errors. Do not register duplicate `pre_tool_call` shell hooks for these three subcommands.

Regression coverage: `tests/test_native_config_protection.py` exercises dispatch order, block-result conversion, cwd forwarding, tool-name sets, and the existing config-protection fail-closed path; `tests/hooks_cli.rs` exercises Rust preflight and hookify matching. These tests use synthetic callback contexts and do not validate behavior inside a live Hermes gateway. Tool-name aliases are explicit in the Python adapter and may need updating if Hermes changes its callback names. Preflight remains lexical rather than shell-aware, and hookify intentionally supports only the Rust evaluator's documented rule subset.

`check-console-log` is merged into `post-observer`; no compatibility wrapper is needed. `stop_lifecycle` was not connected in the shipped sample and was intentionally removed rather than ported. This is a purpose-limited adapter, not a complete port of all ECC hooks: see `INVENTORY.md` for the boundaries. Live Hermes callback integration has not been tested here.
