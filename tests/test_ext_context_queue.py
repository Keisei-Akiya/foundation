from __future__ import annotations

import shutil

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, invoke_hook


def _install_binary(runtime):
    assert BINARY_PATH.is_file(), "build foundation before running this test"
    shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
    (runtime.bin / "foundation").chmod(0o755)


def test_doc_warning_is_queued_then_injected_by_real_hermes_hook():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_binary(runtime)
        invoke_hook("pre_tool_call", tool_name="write_file", args={"path": "/tmp/NOTES.md"}, session_id="doc-session")
        injected = invoke_hook("pre_llm_call", session_id="doc-session", user_message="next")
        assert any(isinstance(item, dict) and "/tmp/NOTES.md" in item.get("context", "") for item in injected)
        assert invoke_hook("pre_llm_call", session_id="doc-session", user_message="again") == []


def test_compact_suggestion_uses_session_tool_count_and_injects_on_next_turn():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_binary(runtime)
        for _ in range(50):
            invoke_hook("pre_tool_call", tool_name="read_file", args={"path": "/tmp/a.rs"}, session_id="count-session")
        injected = invoke_hook("pre_llm_call", session_id="count-session", user_message="next")
        assert any(isinstance(item, dict) and "50 tool calls" in item.get("context", "") for item in injected)
