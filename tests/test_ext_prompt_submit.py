from __future__ import annotations

import shutil
import tempfile
import unittest
from pathlib import Path

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, invoke_hook


class PromptSubmitExtensionTests(unittest.TestCase):
    def setUp(self):
        self.runtime = IsolatedHermesHome(enable_foundation=True)
        self.runtime.__enter__()
        self.addCleanup(self.runtime.__exit__, None, None, None)
        self.assertTrue(BINARY_PATH.is_file(), "build foundation before running this suite")
        binary = self.runtime.bin / "foundation"
        shutil.copy2(BINARY_PATH, binary)
        binary.chmod(0o755)
        self.project = Path(tempfile.mkdtemp(prefix="foundation-prompt-project-"))
        self.addCleanup(shutil.rmtree, self.project, ignore_errors=True)
        self._old_cwd = Path.cwd()
        __import__("os").chdir(self.project)
        self.addCleanup(__import__("os").chdir, self._old_cwd)
        # Hermes caches one plugin manager per process; reload against this isolated home.
        from hermes_cli.plugins import get_plugin_manager

        get_plugin_manager().discover_and_load(force=True)

    def test_real_pre_llm_hook_blocks_matching_prompt_rule(self):
        rules = self.project / ".claude"
        rules.mkdir()
        (rules / "hookify.no-secret.local.md").write_text(
            "---\nname: no-secret\nevent: prompt\naction: block\npattern: secret\n---\nDo not submit secrets.\n",
            encoding="utf-8",
        )
        results = invoke_hook(
            "pre_llm_call", user_message="send SECRET now", is_first_turn=False,
            session_id="test-session", task_id="task", turn_id="turn",
        )
        self.assertTrue(any(isinstance(r, dict) and r.get("action") == "block" for r in results), results)

    def test_real_pre_llm_hook_restores_first_turn_context(self):
        store = self.runtime.home / "plugin-data/foundation"
        store.mkdir(parents=True)
        (store / "session-summary.jsonl").write_text('{"summary":"Resume task 42"}\n', encoding="utf-8")
        results = invoke_hook(
            "pre_llm_call", user_message="hello", is_first_turn=True,
            session_id="new-session", task_id="task", turn_id="turn",
        )
        contexts = [r.get("context", "") for r in results if isinstance(r, dict)]
        self.assertTrue(any("Resume task 42" in c for c in contexts), results)

    def test_real_pre_llm_hook_does_not_restore_context_on_later_turn(self):
        store = self.runtime.home / "plugin-data/foundation"
        store.mkdir(parents=True)
        (store / "session-summary.jsonl").write_text('{"summary":"Resume task 42"}\n', encoding="utf-8")
        results = invoke_hook(
            "pre_llm_call", user_message="next", is_first_turn=False,
            session_id="new-session", task_id="task", turn_id="turn",
        )
        self.assertFalse(any("Resume task 42" in str(r) for r in results), results)


if __name__ == "__main__":
    unittest.main()
