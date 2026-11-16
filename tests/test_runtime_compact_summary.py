from __future__ import annotations

import json
import shutil

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, invoke_hook, registered_hooks


def test_compaction_hook_restores_opted_in_summary_through_hermes_dispatcher():
    assert BINARY_PATH.is_file(), "build foundation before running this suite"
    assert {"post_llm_call", "post_compaction", "pre_llm_call"} <= registered_hooks()
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        data = runtime.home / "plugin-data/foundation"
        data.mkdir(parents=True, exist_ok=True)
        (data / "compact-summary.enabled").touch()
        session = "runtime-compact-summary-test"
        history = [
            {"role": "user", "content": "Implement a feature; export TOKEN=secret; cargo test; /home/private"},
            {"tool_name": "todo", "todos": [
                {"status": "completed", "content": "completed task"},
                {"status": "pending", "content": "unfinished task"},
            ]},
        ]
        invoke_hook("post_llm_call", session_id=session, conversation_history=history)
        # Exercise Hermes' real hook registry and callback dispatch for the committed-event
        # contract; this is not a real context-compressor run.
        invoke_hook(
            "post_compaction",
            session_id=session,
            old_session_id=session,
            kind="auto",
            reason="test-dispatch",
            before_tokens=1000,
            after_tokens=500,
            messages_removed=4,
        )
        records = (data / "compact-summary.jsonl").read_text(encoding="utf-8")
        assert "Purpose: Software implementation." in records
        assert "1 structured tasks remain open." in records
        for forbidden in ("TOKEN=secret", "cargo test", "/home/private"):
            assert forbidden not in records
        results = invoke_hook(
            "pre_llm_call",
            session_id=session,
            user_message="continue",
            is_first_turn=False,
            conversation_history=[],
        )
        assert any(
            isinstance(result, dict)
            and "Purpose: Software implementation." in result.get("context", "")
            and "1 structured tasks remain open." in result.get("context", "")
            for result in results
        )
        assert json.loads((data / "compact-summary.jsonl").read_text().splitlines()[-1])["event"] == "restored"
