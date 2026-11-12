from __future__ import annotations

import json

import pytest

pytest = pytest.importorskip("pytest")

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from adapters.hermes.plugin.hooks_ext import stop_cost_notify


def _callbacks():
    class Context:
        hooks = {}

        def register_hook(self, event, callback):
            self.hooks.setdefault(event, []).append(callback)

    calls = []
    core = type("Core", (), {
        "payload": staticmethod(lambda event, kwargs: {
            "hook_event_name": event,
            **{key: kwargs[key] for key in ("session_id", "api_request_id") if key in kwargs},
        }),
        "run_subcommand": staticmethod(lambda command, payload, timeout=None: calls.append((command, payload))),
    })()
    ctx = Context()
    stop_cost_notify.register(ctx, core)
    return ctx.hooks, calls


def test_post_api_request_and_post_llm_call_dispatch_only_token_data():
    hooks, calls = _callbacks()
    hooks["post_api_request"][0](session_id="s1", api_request_id="r1", response_model="model-x",
                                usage={"prompt_tokens": 7, "completion_tokens": 3,
                                       "cached_tokens": 1})
    for callback in hooks["post_llm_call"]:
        callback(session_id="s1", assistant_response="private text", model="model-x")
    assert [call[0] for call in calls] == ["cost-tracker", "cost-tracker", "desktop-notify"]
    api_payload = calls[0][1]
    assert api_payload["usage"]["prompt_tokens"] == 7
    assert api_payload["response_model"] == "model-x"
    assert calls[1][1]["hook_event_name"] == "post_llm_call"
    assert "assistant_response" not in calls[2][1]
    assert "private text" not in json.dumps(calls)


def test_linux_notification_hook_dispatch_is_best_effort():
    hooks, calls = _callbacks()
    hooks["post_llm_call"][1](session_id="s1", assistant_response="private text", model="model-x")
    assert len(calls) == 1
    assert calls[0][0] == "desktop-notify"
    assert "assistant_response" not in calls[0][1]
