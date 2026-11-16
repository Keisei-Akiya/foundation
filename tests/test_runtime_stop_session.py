from __future__ import annotations

import json
import shlex
import stat

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, invoke_hook, registered_hooks


def test_stop_and_session_hooks_run_through_hermes_plugin_manager():
    assert BINARY_PATH.is_file(), "build foundation before running this suite"
    expected_events = {"pre_verify", "post_llm_call", "on_session_finalize"}
    assert expected_events <= registered_hooks()

    with IsolatedHermesHome(enable_foundation=True) as runtime:
        summary_store = runtime.home / "plugin-data/foundation"
        summary_store.mkdir(parents=True, exist_ok=True)
        (summary_store / "session-summary.enabled").write_text("", encoding="utf-8")
        calls_path = runtime.root / "foundation-calls.txt"
        real_binary = shlex.quote(str(BINARY_PATH))
        wrapper = runtime.bin / "foundation"
        wrapper.write_text(
            "#!/bin/sh\n"
            f"printf '%s\\n' \"$1\" >> {shlex.quote(str(calls_path))}\n"
            f"exec {real_binary} \"$@\"\n",
            encoding="utf-8",
        )
        wrapper.chmod(0o755)

        project = runtime.root / "project"
        rules = project / ".claude"
        rules.mkdir(parents=True)
        (rules / "hookify.stop.local.md").write_text(
            "---\nevent: stop\naction: block\npattern: TODO\n---\nResolve TODOs first.\n",
            encoding="utf-8",
        )
        (project / "app.ts").write_text("console.log('debug');\n", encoding="utf-8")

        verify_results = invoke_hook(
            "pre_verify",
            changed_paths=["app.ts"],
            final_response="TODO remains",
            attempt=0,
            cwd=str(project),
        )
        assert any(
            isinstance(result, dict)
            and result.get("action") == "continue"
            and "app.ts" in result.get("message", "")
            for result in verify_results
        )

        history: list[dict[str, object]] = [
            {"role": "user", "content": f"private prompt {i} /home/private export TOKEN=secret cargo test"}
            for i in range(10)
        ]
        history.append({"role": "assistant", "tool_name": "todo", "todos": [{"status": "pending", "content": "private todo"}, {"status": "completed", "content": "private completed todo"}]})
        invoke_hook(
            "post_llm_call",
            session_id="runtime-stop-session-test",
            conversation_history=history,
            assistant_response="private response",
            model="test-model",
            platform="cli",
        )
        invoke_hook(
            "on_session_finalize",
            session_id="runtime-stop-session-test",
            platform="cli",
            reason="exit",
        )

        calls = calls_path.read_text(encoding="utf-8").splitlines()
        expected_calls = [
            "stop-hookify",
            "format-typecheck",
            "check-console-log",
            "session-end",
            "evaluate-session",
            "session-hooks",
        ]
        position = -1
        for expected in expected_calls:
            position = calls.index(expected, position + 1)

        data = runtime.home / "plugin-data/foundation"
        summary_text = (data / "session-summary.jsonl").read_text(encoding="utf-8")
        summary = json.loads(summary_text.splitlines()[0])
        assert summary["event"] == "stop:session-end"
        assert summary["session_id"] == "runtime-stop-session-test"
        assert "Purpose: Testing and verification." in summary["summary"]
        assert "Progress: 1/2 structured tasks completed; 1 tool operations recorded." in summary["summary"]
        assert "Unfinished: 1 structured tasks remain open." in summary["summary"]
        for forbidden in ("private prompt", "private response", "/home/", "TOKEN=secret", "cargo test"):
            assert forbidden not in summary_text

        second_session = "runtime-second-session-test"
        second_turn_results = invoke_hook(
            "pre_llm_call",
            session_id=second_session,
            is_first_turn=True,
            turn_index=0,
            user_message="continue",
            cwd=str(project),
        )
        assert any(
            isinstance(result, dict)
            and "Purpose:" in result.get("context", "")
            and "Unfinished: 1 structured tasks remain open." in result.get("context", "")
            for result in second_turn_results
        )

        evaluation_text = (data / "session-evaluation.jsonl").read_text(encoding="utf-8")
        evaluation = json.loads(evaluation_text.splitlines()[0])
        assert evaluation["event"] == "stop:evaluate-session"
        assert evaluation["status"] == "eligible_for_manual_review"

        marker_path = runtime.home / "plugin-data/foundation/session-markers.jsonl"
        marker_text = marker_path.read_text(encoding="utf-8")
        marker = json.loads(marker_text.splitlines()[0])
        assert marker["event"] == "session:end:marker"
        assert marker["session_id"] == "runtime-stop-session-test"
        assert stat.S_IMODE(marker_path.stat().st_mode) == 0o600


def test_next_session_receives_notice_for_interrupted_prior_session():
    assert BINARY_PATH.is_file(), "build foundation before running this suite"
    assert {"on_session_start", "on_session_end", "on_session_finalize", "pre_llm_call"} <= registered_hooks()

    with IsolatedHermesHome(enable_foundation=True) as runtime:
        (runtime.bin / "foundation").symlink_to(BINARY_PATH)
        invoke_hook("on_session_start", session_id="interrupted-session", platform="cli")
        invoke_hook(
            "on_session_end",
            session_id="interrupted-session",
            completed=False,
            interrupted=True,
            turn_exit_reason="keyboard_interrupt",
            platform="cli",
        )

        results = invoke_hook(
            "pre_llm_call",
            session_id="next-session",
            is_first_turn=True,
            turn_index=0,
            user_message="continue",
            cwd=str(runtime.root),
        )
        assert any(
            isinstance(result, dict)
            and "previous session was interrupted" in result.get("context", "")
            and "last turn may be incomplete" in result.get("context", "")
            for result in results
        )

        marker_path = runtime.home / "plugin-data/foundation/session-markers.jsonl"
        records = [json.loads(line) for line in marker_path.read_text(encoding="utf-8").splitlines()]
        assert records[0]["event"] == "session:start:marker"
        assert records[1]["event"] == "session:turn:end"
        assert records[1]["interrupted"] is True
        assert records[1]["turn_exit_reason"] == "keyboard_interrupt"
