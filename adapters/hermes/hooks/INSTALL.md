# Install the Rust config-protection hook

This hook blocks Hermes file-edit tools from modifying an *existing* protected lint/format configuration file. It allows new protected filenames and unrelated files. It is a narrow runtime guard, not OS isolation, and it does not cover terminal commands or other tools outside the configured matcher.

## Requirements

- Run on a supported Hermes installation that includes shell hooks and `hermes hooks list`, `hermes hooks test`, and `hermes hooks doctor`.
- Have Rust/Cargo available. `cargo build --release` compiles the binary; Cargo may need network access to fetch dependencies on the first build.
- Build from this checkout. The installer script does not install Rust, alter Hermes configuration, change hook approvals, or restart a gateway.
- `HERMES_HOME` selects the target Hermes home (including a profile home). If unset, the script targets `~/.hermes`. Always pass an isolated directory while testing.

## Build and stage

From any working directory:

```sh
adapters/hermes/hooks/install-config-protection.sh
```

This builds `target/release/ecc-hermes-hooks` and prints the exact hook registration using that binary path. It does not copy or install anything into Hermes. To stage a copy inside the selected Hermes home, run:

```sh
adapters/hermes/hooks/install-config-protection.sh --install
```

For isolated testing, explicitly choose a temporary home:

```sh
adapters/hermes/hooks/install-config-protection.sh --install --hermes-home /path/to/isolated-hermes-home
```

The copy is placed at `<HERMES_HOME>/bin/ecc-hermes-hooks`. No config file is changed by either invocation.

## Apply explicitly

1. Review the command and hook source, especially `src/main.rs`.
2. Back up the intended Hermes `config.yaml` using your normal backup process.
3. Manually add only this entry under the existing top-level `hooks:` mapping. Preserve all existing entries and indentation; do not replace the whole config with the snippet.

```yaml
hooks:
  pre_tool_call:
    - command: "'/absolute/path/to/HERMES_HOME/bin/ecc-hermes-hooks' config-protection"
      matcher: "^(write_file|edit_file|patch|apply_patch)$"
      timeout: 10
      fail_closed: false
```

If building without `--install`, use the printed `target/release/ecc-hermes-hooks` path instead. The `matcher` is Hermes' full-match regex. `pre_tool_call` is blocking-capable; the Rust program responds with `{"decision":"block","reason":"..."}` for a matching attempted edit to an existing protected file. The program otherwise emits no stdout. `fail_closed: false` preserves Hermes' default fail-open behavior if the process fails; this means a missing/broken hook does not guarantee protection.

Do not set `hooks_auto_accept: true` globally. On the first Hermes CLI use, review the exact command/event consent prompt and approve only if it matches the inspected command. Hermes records the approval per event and command in its hook allowlist. Do not use `--accept-hooks` or `HERMES_ACCEPT_HOOKS=1` as a shortcut to approve unrelated hooks.

## Verify

After applying the config and approving this exact event/command pair through the intended Hermes CLI prompt, run `hermes hooks list` and `hermes hooks doctor`. Confirm the hook is configured and allowlisted and doctor reports it runnable without errors. The approval prompt is the recommended least-privilege path; the one-time `--accept-hooks` flag approves all currently configured shell hooks for that run, so review the whole configured hook set before using it. Do not enable blanket `hooks_auto_accept`.

Test both deny and allow cases with harmless files in a disposable project; the protected filename must already exist for the deny case. For `hermes hooks test`, its payload file overrides synthetic kwargs, so put the file path under `args`:

```json
{"args":{"path":"/absolute/path/to/disposable-project/.prettierrc"}}
```

Use `hermes hooks test pre_tool_call --for-tool write_file --payload-file /path/to/payload.json`. This CLI test is synthetic; it uses Hermes' hook runner and response parser, but does not establish that a live Hermes session invokes the hook. A real Hermes run through a fresh CLI session is needed to establish end-to-end registration, consent, and blocking. This project does not restart any long-running Hermes gateway automatically. If the target is a gateway profile, arrange its reload/restart through the operator's normal procedure after reviewing the resulting configuration.

## Roll back

1. Remove only this hook entry from the intended `config.yaml`, preserving all other configuration.
2. Revoke its exact approval if desired: `hermes hooks revoke "/absolute/path/to/HERMES_HOME/bin/ecc-hermes-hooks config-protection"`. Run this under the same Hermes home/profile. Check `hermes hooks list` afterward.
3. Remove the staged executable only if no other configuration uses it: `rm -- "/absolute/path/to/HERMES_HOME/bin/ecc-hermes-hooks"`.

The script's build artifact remains in this checkout's `target/release/`; remove it only if appropriate for the working tree. Rollback does not restore config automatically because installation never edits config and an automatic restore could overwrite unrelated user changes.

## Isolated validation

For a safe end-to-end registration experiment, use a disposable Hermes installation/profile configured to read its home from an isolated `HERMES_HOME`. Stage the binary there, configure only the snippet above in that isolated home's config, and exercise the real hook CLI/runtime. Confirm the test home is isolated before running any Hermes command that might create state. The install script itself can be tested without Hermes by passing a temporary home and checking that only its `bin/ecc-hermes-hooks` is created.
