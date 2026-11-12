import json
import shutil
import unittest

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, invoke_hook, registered_hooks


class StopSessionExtensionTests(unittest.TestCase):
    def setUp(self):
        self.runtime = IsolatedHermesHome(enable_foundation=True)
        self.runtime.__enter__()
        self.addCleanup(self.runtime.__exit__, None, None, None)
        self.assertTrue(BINARY_PATH.is_file(), "build foundation before running this suite")
        binary = self.runtime.bin / "foundation"
        shutil.copy2(BINARY_PATH, binary)
        binary.chmod(0o755)

    def test_post_llm_call_is_a_real_event_and_records_private_data_free_metadata(self):
        self.assertIn("post_llm_call", registered_hooks())
        history = [{"role": "user", "content": f"private user {i}"} for i in range(10)]
        invoke_hook(
            "post_llm_call",
            session_id="session-1",
            conversation_history=history,
            assistant_response="private assistant response",
            model="test-model",
            platform="cli",
        )
        summary = self.runtime.home / "plugin-data/foundation/session-summary.jsonl"
        evaluation = self.runtime.home / "plugin-data/foundation/session-evaluation.jsonl"
        record = json.loads(summary.read_text(encoding="utf-8").splitlines()[0])
        marker = json.loads(evaluation.read_text(encoding="utf-8").splitlines()[0])
        self.assertEqual(record["user_message_count"], 10)
        self.assertEqual(marker["status"], "eligible_for_manual_review")
        self.assertNotIn("private user", summary.read_text(encoding="utf-8"))
        self.assertNotIn("private assistant response", summary.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
