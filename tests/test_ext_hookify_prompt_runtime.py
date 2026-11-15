from __future__ import annotations

import shutil
import tempfile
import unittest
from pathlib import Path

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, invoke_hook


class HookifyPromptRuntimeHermesTests(unittest.TestCase):
    def setUp(self):
        self.runtime = IsolatedHermesHome(enable_foundation=True)
        self.runtime.__enter__()
        self.addCleanup(self.runtime.__exit__, None, None, None)
        self.assertTrue(BINARY_PATH.is_file(), "build foundation before running this suite")
        binary = self.runtime.bin / "foundation"
        shutil.copy2(BINARY_PATH, binary)
        binary.chmod(0o755)
        self.project = Path(tempfile.mkdtemp(prefix="foundation-hookify-prompt-"))
        self.addCleanup(shutil.rmtree, self.project, ignore_errors=True)
        self._old_cwd = Path.cwd()
        __import__("os").chdir(self.project)
        self.addCleanup(__import__("os").chdir, self._old_cwd)
        from hermes_cli.plugins import get_plugin_manager

        get_plugin_manager().discover_and_load(force=True)

    def test_real_hermes_pre_llm_hook_evaluates_hookify_all_rule(self):
        rules = self.project / ".claude"
        rules.mkdir()
        (rules / "hookify.all.local.md").write_text(
            "---\nevent: all\naction: warn\npattern: credential\n---\nCheck credential handling.\n",
            encoding="utf-8",
        )
        results = invoke_hook(
            "pre_llm_call",
            user_message="review credential handling",
            is_first_turn=False,
            session_id="fixture-session",
            task_id="fixture-task",
            turn_id="fixture-turn",
        )
        contexts = [result.get("context", "") for result in results if isinstance(result, dict)]
        self.assertTrue(any("Check credential handling." in context for context in contexts), results)

    def test_real_hermes_pre_llm_hook_fails_open_for_malformed_rule(self):
        rules = self.project / ".claude"
        rules.mkdir()
        (rules / "hookify.invalid.local.md").write_text(
            "---\nevent: prompt\naction: block\npattern: [\n---\ninvalid\n",
            encoding="utf-8",
        )
        results = invoke_hook(
            "pre_llm_call",
            user_message="ordinary prompt",
            is_first_turn=False,
            session_id="fixture-session",
            task_id="fixture-task",
            turn_id="fixture-turn",
        )
        self.assertFalse(
            any(isinstance(result, dict) and result.get("action") == "block" for result in results),
            results,
        )


if __name__ == "__main__":
    unittest.main()
