from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))
from adapters.hermes.hooks import post_dispatch  # noqa: E402


def test_hermes_post_payload_runs_only_read_only_edit_observers(tmp_path: Path) -> None:
    source = tmp_path / "app.tsx"
    source.write_text('console.log("debug"); body { color: #000; }', encoding="utf-8")
    payload = {
        "hook_event_name": "post_tool_call",
        "tool_name": "write_file",
        "tool_input": {"path": "app.tsx"},
        "cwd": str(tmp_path),
    }
    results = post_dispatch.evaluate(payload)
    assert len(results) == 2
    assert "Design review reminder" in results[0]
    assert "Post-edit warning" in results[1]
    assert source.read_text(encoding="utf-8") == 'console.log("debug"); body { color: #000; }'


def test_ignores_non_post_event_and_non_edit_tool(tmp_path: Path) -> None:
    source = tmp_path / "app.js"
    source.write_text("console.log('x')", encoding="utf-8")
    payload = {"hook_event_name": "pre_tool_call", "tool_name": "write_file", "tool_input": {"path": str(source)}}
    assert post_dispatch.evaluate(payload, tmp_path) == []
    payload["hook_event_name"] = "post_tool_call"
    payload["tool_name"] = "terminal"
    assert post_dispatch.evaluate(payload, tmp_path) == []


def test_missing_or_oversized_file_is_suppressed(tmp_path: Path) -> None:
    assert post_dispatch.check_console_log("patch", {"path": "missing.ts"}, tmp_path) == []
    source = tmp_path / "large.ts"
    source.write_bytes(b"x" * (post_dispatch.MAX_FILE_BYTES + 1))
    assert post_dispatch.check_console_log("patch", {"path": "large.ts"}, tmp_path) == []


def test_cli_uses_hermes_payload_and_emits_diagnostics_on_stderr(monkeypatch, capsys, tmp_path: Path) -> None:
    source = tmp_path / "main.js"
    source.write_text("console.info('debug')", encoding="utf-8")
    payload = '{"hook_event_name":"post_tool_call","tool_name":"patch","tool_input":{"path":"main.js"},"cwd":' + repr(str(tmp_path)).replace("'", '"') + '}'
    monkeypatch.setattr("sys.stdin", __import__("io").StringIO(payload))
    assert post_dispatch.main() == 0
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "Post-edit warning" in captured.err


def test_unknown_event_and_malformed_json_fail_open(monkeypatch, capsys) -> None:
    monkeypatch.setattr("sys.stdin", __import__("io").StringIO("not json"))
    assert post_dispatch.main() == 0
    assert capsys.readouterr().err == ""
