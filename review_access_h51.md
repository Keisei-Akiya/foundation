# foundation #51: user-facing cost access review

## Issue and acceptance

GitHub issue: [#51 `hook: stop:cost-tracker`](https://github.com/Keisei-Akiya/foundation/issues/51). Its requirement is explicit: session-level token counts and cost must be inspectable; when cost cannot be calculated, display the reason. A private JSONL write alone does not satisfy this. The issue also requires documenting price configuration in `adapters/hermes/hooks/INVENTORY.md`, fail-open behavior, a constructed-JSON CLI test, and an isolated real-Hermes session-total check.

## Finding and implementation

There was no user-facing cost-report command in the branch before this change. Added `foundation cost-report`, which reads the existing state store and prints session IDs, token totals, model, and either estimated USD cost or an unavailable status/reason. It reads no prompts or conversation text and does not expose them. Empty/unreadable data gives a readable no-data/error response rather than dumping the JSONL.

Price configuration is explicit and local: `$HERMES_HOME/plugin-data/foundation/model-pricing.json`. The new `docs/cost-tracker.md` describes the config shape, rate units, command, unknown-model behavior, data location, and privacy limits. No provider price list or automatic price fetch was added; displayed amounts are estimates against user-maintained rates, not invoice totals.

Usage handling considers the differing cache conventions: if an OpenAI-style prompt total includes `cached_tokens`, cached input is removed from regular input before pricing; when a provider reports net input plus separate cache-read/cache-write counts, regular input is not subtracted again. Cache counts are priced separately. Hermes documents `post_api_request` as observer-only after successful provider-response normalization, with `usage` as accounting data (`/home/rocky/.hermes/hermes-agent/website/docs/user-guide/features/hooks.md`, lines 1898–1900). The cost hook is exercised in the isolated runtime test; live billing accuracy for every provider/model is not established.

## Evidence

- `cargo test --locked` — passed (entire Rust test suite).
- `cargo clippy --all-targets --all-features --locked -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- Built `foundation` with `cargo build --locked --bin foundation` — passed.
- Hermes runtime tests, with isolated Hermes checkout and built binary:
  `uv run --project /home/rocky/.hermes/hermes-agent --with pytest python -m pytest -q tests/test_runtime_post_tool.py tests/test_runtime_prompt_cost.py` — **9 passed**. The cost runtime test verifies the real hook records a session total and that the public CLI displays that session's no-price reason. A sentinel prompt string is asserted absent from the state file and CLI output.
- The CLI integration test uses constructed JSON in a temporary state store and checks session/token output plus a cost amount. Unit tests cover usage normalization, cache handling, unknown prices, and fail-open behavior.
- `git diff --check` — passed.
- CI (`.github/workflows/ci.yml`) includes `cargo test --locked`, both Hermes runtime test files, all-target/all-feature clippy with warnings denied, formatting, shell syntax checks, and a musl build. Tests, clippy, format, and the runtime suite were run locally; shell syntax and musl CI steps were not rerun because these changes do not alter those scripts/build targets.

## Files changed

- `src/cost_tracker.rs`, `src/main.rs`: report command and usage/accounting logic.
- `tests/cost_notify_cli.rs`: constructed-state CLI amount test.
- `tests/test_runtime_prompt_cost.py`: assert real-Hermes CLI visibility and no prompt disclosure.
- `docs/cost-tracker.md`: user instructions/pricing/privacy.
- `adapters/hermes/hooks/INVENTORY.md`: updated #51 purpose and status to reflect the report.

## Remaining boundaries

- Prices must be supplied by the user and kept current; unsupported/unknown model prices produce an explicit unavailable reason. Flat-rate plans do not imply a per-token cost.
- Amounts are estimates for tracked API usage, not provider invoice reconciliation. Provider-specific billing semantics can change; validate rates and cache semantics for the configured model/provider.
- Existing observer behavior and its scope are unchanged; no attempt was made to reproduce ECC Stop control behavior or to track non-hook/auxiliary usage.

## Git state

Working branch: `feat/hook-51-cost-tracker`, tracking `origin/feat/hook-51-cost-tracker`. Changes were not yet committed or pushed when this review was written.
