# Hermes runtime integration tests

`mise run runtime-test` runs the existing native config-protection tests and the Hermes dispatcher smoke tests. It never edits the real `~/.hermes/config.yaml`; each case creates a temporary `HERMES_HOME`, enables a symlink to this repository's plugin and places the built `foundation` binary at `<HERMES_HOME>/bin/foundation`.

The tests import Hermes' own `hermes_cli.plugins.invoke_hook` and `VALID_HOOKS`. Point `HERMES_PYTHON` at the Python interpreter in the Hermes runtime environment and build foundation first:

```sh
flock /home/rocky/.hermes/cache/scratch/wf_foundation27_8ed0ea57/cargo.lock env CARGO_TARGET_DIR=/home/rocky/repo/foundation-native-plugin/target cargo build --locked -j 2 --bin foundation
export FOUNDATION_TEST_BINARY=/home/rocky/repo/foundation-native-plugin/target/debug/foundation
export HERMES_AGENT_ROOT=/home/rocky/.hermes/hermes-agent
export HERMES_PYTHON=/path/to/hermes/venv/bin/python
mise run runtime-test
```

The task runs `uv run --with pytest --python "$HERMES_PYTHON" ...`, so pytest is made available in an ephemeral uv environment while the interpreter and dependencies come from Hermes. If Hermes is unavailable, the dispatcher smoke module skips; the existing plugin unit tests remain runnable. CI environments without the Hermes source/runtime cannot prove callback delivery and must not report that integration check as passed.

The smoke test reads `VALID_HOOKS` at collection time and registers a probe callback for each live event with Hermes' plugin dispatcher, then calls Hermes' real `invoke_hook` and checks the callback result. As Hermes adds events, they are automatically included. A separate check confirms Hermes discovers the actual `foundation` plugin under the isolated home.

The isolated check `HERMES_HOME=<temporary-home> hermes hooks doctor` returned `No shell hooks configured — nothing to check.` even when the foundation native plugin was enabled. Hermes `hermes_cli/hooks.py` `_cmd_doctor` calls `shell_hooks.iter_configured_hooks(load_config())` and then prints that message when the shell-hook list is empty. This doctor command only validates shell-hook configuration, not native plugin loading or callback dispatch. Use this pytest harness for plugin callback registration and dispatch instead. The example shell-hook entries remain separately selectable in `adapters/hermes/hooks/hooks.example.yaml`.
