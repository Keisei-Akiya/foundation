"""Wire-level tests for the remaining upstream ECC shell hook.

Hermes shell hooks receive ``hook_event_name``, ``tool_name`` and a dict
``tool_input`` on stdin. These tests exercise the console observer with those
production payloads and check its non-blocking output.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


HOOKS = Path(__file__).parents[1] / "adapters" / "hermes" / "hooks"


def run_hook(script: str, event: str, tool: str, tool_input: dict[str, str], cwd: Path):
    payload = {
        "hook_event_name": event,
        "tool_name": tool,
        "tool_input": tool_input,
        "session_id": "test-session",
        "cwd": str(cwd),
        "extra": {},
    }
    return subprocess.run(
        [sys.executable, str(HOOKS / script)],
        input=json.dumps(payload), text=True, capture_output=True, cwd=cwd,
        env={**os.environ, "HERMES_HOME": str(cwd / "hermes-home")},
        check=False,
    )


class ExistingHermesHooksTests(unittest.TestCase):
    def test_console_log_warns_after_matching_file_tool_but_never_blocks(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "app.ts"
            source.write_text("console.log('debug');\n")
            result = run_hook("check-console-log.py", "post_tool_call", "write_file", {"path": str(source)}, root)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("console.log() present", result.stderr)

    def test_console_log_skips_nonmatching_tool_and_non_js_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "app.ts"
            source.write_text("console.log('debug');\n")
            result = run_hook("check-console-log.py", "post_tool_call", "terminal", {"path": str(source)}, root)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stderr, "")
            text_file = root / "app.py"
            text_file.write_text("console.log('debug');\n")
            result = run_hook("check-console-log.py", "post_tool_call", "write_file", {"path": str(text_file)}, root)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stderr, "")


if __name__ == "__main__":
    unittest.main()
