//! Detached `post_tool_call` worker; only routes existing opt-in equivalents.
use serde_json::Value;

pub fn run(payload: &Value) {
    if payload.get("hook_event_name").and_then(Value::as_str) != Some("post_tool_call") {
        return;
    }

    // ECC's asynchronous observer and skill tracker have native counterparts.
    // The observer applies its own opt-in config gate; lifecycle applies its
    // ECC_HERMES_SKILL_TRACK opt-in and skill/tool validation.
    super::observe::run(payload);
    super::lifecycle::lifecycle(payload);

    // The upstream quality-gate and bash-dispatcher child hooks are not ported
    // in this binary yet; do not synthesize behavior or persist tool contents.
}
