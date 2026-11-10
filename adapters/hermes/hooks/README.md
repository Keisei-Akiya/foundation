# Hermes shell hooks

Hermes shell hooks are user-configured entries under `hooks:` in `~/.hermes/config.yaml`; each `command` is executed as an argv (not through a shell), and the subprocess receives the event JSON on stdin. See [`hooks.example.yaml`](hooks.example.yaml) for individually commented opt-ins. Copy only the entries you intend to enable, and replace `FOUNDATION_ROOT` with the absolute path to this checkout. Quote the script path in the command so paths containing spaces remain one argv item. No user configuration is edited by this adapter.

## Enable one hook at a time

1. Inspect the script and the exact command in `hooks.example.yaml`.
2. Substitute the checkout's absolute path for `FOUNDATION_ROOT`; retain the quotes around the script path.
3. Copy only that hook entry into `~/.hermes/config.yaml` under `hooks:`. Leave the other examples commented out.
4. Approve the exact event/command pair at Hermes' prompt, or use Hermes' explicit `--accept-hooks`/`HERMES_ACCEPT_HOOKS=1` opt-in. Do not enable `hooks_auto_accept` globally just to install one example.
5. Run `hermes hooks list` and exercise the matching event with a safe test input.

The scripts are independent alternatives, not an all-or-nothing bundle. `preflight_policies.py` only blocks its documented narrow lexical cases. `hookify_runtime.py` applies configured rules at `pre_tool_call`; inspect its rule/config inputs before enabling. `post_dispatch.py` is a post-tool observer and cannot block the action that already ran. `config-protection.py` checks selected config changes, and `check-console-log.py` is a separate opt-in helper. `lifecycle.py` includes extra environment-based controls; enable it only after reviewing those requirements. `stop_lifecycle.py` is helper code only and is not wired by this sample.

Hermes captures shell-hook stderr only in debug logging (`agent/shell_hooks.py`); stderr messages are not user-visible warnings. The console checker therefore cannot reliably alert a user. Do not rely on stderr for alerts. A hook's supported blocking directive is emitted as JSON on stdout for blocking-capable events.

`config-protection.py` and `check-console-log.py` remain verbatim upstream copies. Hermes exposes the editor as `edit_file`, while these originals recognize `edit`; the example commands use `hermes-tool-compat.py` to translate that tool name without modifying either copy. The config-protection command is registered at `pre_tool_call` so it can block before an edit. The console observer is registered at `post_tool_call` and matches file-editing tools, not `terminal`.

## Verify the integration

From the foundation checkout, run:

```sh
uv run --no-project --with pytest --with ruamel.yaml pytest -q tests/test_ecc_integration.py
```

The integration checks need a Hermes source checkout containing `agent/shell_hooks.py`. Set `HERMES_AGENT_DIR` to its root to select it explicitly; otherwise the test searches `~/.hermes/hermes-agent` and `hermes-agent` folders alongside the test's parent directories. If none is found, only tests that need the Hermes source are skipped; the sample YAML check still runs. To exercise the Hermes integration, configure a valid checkout rather than treating a skip as a pass of those checks.

The integration tests import Hermes' real `agent/shell_hooks.py`, register Python-constructed hook configurations against an isolated temporary `HERMES_HOME`, then invoke registered callbacks for blocking and permitted inputs. They do not parse/load the commented sample hook registrations from `hooks.example.yaml`; the sample file is checked separately for valid YAML (all registrations remain commented). The tests do not modify the real Hermes home or user config.

Hermes adapter integration documentation and sample registrations are in English so upstream syntax and event names stay explicit.
