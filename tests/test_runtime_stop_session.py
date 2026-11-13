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

        history = [{"role": "user", "content": f"private prompt {i}"} for i in range(10)]
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
        assert summary["user_message_count"] == 10
        assert "private prompt" not in summary_text
        assert "private response" not in summary_text

        evaluation_text = (data / "session-evaluation.jsonl").read_text(encoding="utf-8")
        evaluation = json.loads(evaluation_text.splitlines()[0])
        assert evaluation["event"] == "stop:evaluate-session"
        assert evaluation["status"] == "eligible_for_manual_review"

        marker_path = runtime.home / "plugin-data/ecc-hooks/session-markers.jsonl"
        marker_text = marker_path.read_text(encoding="utf-8")
        marker = json.loads(marker_text.splitlines()[0])
        assert marker["event"] == "session:end:marker"
        assert marker["session_id"] == "runtime-stop-session-test"
        assert stat.S_IMODE(marker_path.stat().st_mode) == 0o600
