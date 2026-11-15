# Session cost report

Foundation's Hermes cost hook stores provider usage and per-session totals under:

```text
$HERMES_HOME/plugin-data/foundation/cost_tracker.jsonl
```

After sessions have completed, run:

```sh
foundation cost-report
```

It prints the latest total per session: session token, contributing model(s), input/output token counts, and estimated USD cost. Sessions using more than one model list all contributing models (the total itself is still one session-wide aggregate). If a stored session total is missing required fields, it is marked `invalid/incomplete record` rather than rendered as zero tokens. If pricing is missing, it prints `unavailable` with a reason. The report reads only session totals; prompts, assistant replies, and tool content are neither stored by this hook nor emitted by the report. An empty or unavailable data store prints `No session cost records yet.`

## Configure model rates

Create `$HERMES_HOME/plugin-data/foundation/model-pricing.json`. Keys must exactly match the model name reported by Hermes (`response_model`, falling back to `model`). Rates are USD per million tokens:

```json
{
  "claude-sonnet-4-6": {
    "input_usd_per_million": 3.0,
    "output_usd_per_million": 15.0,
    "cache_write_usd_per_million": 3.75,
    "cache_read_usd_per_million": 0.3
  }
}
```

Input and output rates are required. Cache rates are required only when the corresponding cache token count is non-zero. Rates are intentionally user-supplied: provider prices vary by plan, region, date, and negotiated terms. Missing or invalid rates yield an unavailable cost instead of an invented zero. Review/update the file when your provider's pricing changes.

These are estimates based on the usage reported in Hermes `post_api_request` (`usage`, `response_model`/`model`). The checked Hermes hook contract's fixture uses `input_tokens`/`output_tokens`; the tracker also accepts `prompt_tokens`/`completion_tokens`, Anthropic-style cache creation/read fields, and `cached_tokens`. When both `prompt_tokens` and `cached_tokens` are present, cached tokens are subtracted from the prompt total before applying normal input pricing, then billed using the configured cache-read rate. Provider-specific usage variants that do not fit those semantics may need normalization; compare reported estimates with the provider's usage/billing page. This tool cannot guarantee provider invoices, discounts, or subscription-plan charges.
