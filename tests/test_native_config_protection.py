import importlib.util
import os
import tempfile
import unittest
from pathlib import Path

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, REPO_ROOT

PLUGIN_PATH = Path(__file__).parents[1] / "adapters/hermes/plugin/__init__.py"
SPEC = importlib.util.spec_from_file_location("foundation_plugin", PLUGIN_PATH)
assert SPEC is not None and SPEC.loader is not None
plugin = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(plugin)


class HookContext:
    def __init__(self):
        self.hooks = {}

    def register_hook(self, name, callback):
        self.hooks[name] = callback


class NativeConfigProtectionTests(unittest.TestCase):
    def setUp(self):
        self.runtime = IsolatedHermesHome()
        self.runtime.__enter__()
        self.addCleanup(self.runtime.__exit__, None, None, None)
        self.home = self.runtime.root
        self.hermes_home = self.runtime.home
        self.bin_dir = self.runtime.bin

    def _install_real_binary(self):
        self.assertTrue(BINARY_PATH.is_file(), "build foundation before running this suite")
        if not (self.bin_dir / "foundation").exists():
            import shutil
            shutil.copy2(BINARY_PATH, self.bin_dir / "foundation")
            (self.bin_dir / "foundation").chmod(0o755)

    def _callback(self):
        # Exercise the core dispatcher directly. register() also loads extension
        # hooks, which may register the same event and overwrite this test double's
        # single-callback slot; dispatch composition belongs to Hermes integration tests.
        return plugin.on_pre_tool_call

    def test_real_binary_allows_unrelated_edit(self):
        self._install_real_binary()
        callback = self._callback()
        result = callback(tool_name="write_file", args={"path": str(self.home / "src/main.rs")})
        self.assertIsNone(result)

    def test_real_binary_blocks_existing_protected_config(self):
        self._install_real_binary()
        protected = self.home / "eslint.config.js"
        protected.write_text("export default {};\n", encoding="utf-8")
        callback = self._callback()
        result = callback(tool_name="write_file", args={"path": str(protected)})
        self.assertEqual(result["action"], "block")
        self.assertIn("linter/formatter", result["message"])

    def test_missing_binary_fails_closed_for_edit(self):
        callback = self._callback()
        result = callback(tool_name="write_file", args={"path": str(self.home / "src/main.rs")})
        self.assertEqual(result["action"], "block")
        self.assertIn("failed", result["message"])

    def test_nonzero_binary_failure_fails_closed(self):
        fake_binary = self.bin_dir / "foundation"
        fake_binary.write_text("#!/bin/sh\ncat >/dev/null\nexit 1\n", encoding="utf-8")
        fake_binary.chmod(0o755)
        callback = self._callback()
        result = callback(tool_name="patch", args={"file_path": str(self.home / "src/main.rs")})
        self.assertEqual(result["action"], "block")
        self.assertIn("exit code 1", result["message"])

    def test_empty_successful_binary_allows_edit(self):
        fake_binary = self.bin_dir / "foundation"
        fake_binary.write_text("#!/bin/sh\ncat >/dev/null\nexit 0\n", encoding="utf-8")
        fake_binary.chmod(0o755)
        callback = self._callback()
        result = callback(tool_name="patch", args={"file_path": str(self.home / "src/main.rs")})
        self.assertIsNone(result)

    def test_timeout_fails_closed(self):
        fake_binary = self.bin_dir / "foundation"
        fake_binary.write_text("#!/bin/sh\ncat >/dev/null\nsleep 2\n", encoding="utf-8")
        fake_binary.chmod(0o755)
        callback = self._callback()
        previous_timeout = plugin._CONFIG_PROTECTION_TIMEOUT_SECONDS
        plugin._CONFIG_PROTECTION_TIMEOUT_SECONDS = 0.01
        try:
            result = callback(tool_name="patch", args={"file_path": str(self.home / "src/main.rs")})
        finally:
            plugin._CONFIG_PROTECTION_TIMEOUT_SECONDS = previous_timeout
        self.assertEqual(result["action"], "block")
        self.assertIn("timed out", result["message"])

    def test_all_ecc_edit_tool_names_are_dispatched(self):
        from unittest.mock import patch

        callback = self._callback()
        with patch.object(plugin, "run_subcommand", return_value=None) as run_subcommand:
            for tool_name in ("patch", "write_file", "edit", "edit_file", "write", "apply_patch", "str_replace_editor"):
                self.assertIsNone(callback(tool_name=tool_name, args={"path": str(self.home / "src/main.rs")}))
        self.assertEqual(run_subcommand.call_count, 28)
        self.assertEqual(
            [call.args[0] for call in run_subcommand.call_args_list],
            [name for _ in range(7) for name in ("observe", "governance-capture", "hookify", "config-protection")],
        )

    def test_non_edit_tool_does_not_run_config_protection(self):
        from unittest.mock import patch

        callback = self._callback()
        with patch.object(plugin, "run_subcommand", return_value=None) as run_subcommand:
            self.assertIsNone(callback(tool_name="read_file", args={"path": "eslint.config.js"}))
        self.assertEqual([call.args[0] for call in run_subcommand.call_args_list], ["observe", "governance-capture", "hookify"])
    def test_bash_dispatch_runs_preflight_then_hookify_and_blocks(self):
        from unittest.mock import patch

        callback = self._callback()
        with patch.object(
            plugin,
            "run_subcommand",
            side_effect=[None, None, None, {"action": "block", "message": "hookify denied"}],
        ) as run_subcommand:
            result = callback(tool_name="terminal", args={"command": "rm -rf /tmp/x"}, cwd="/project")
        self.assertEqual(result, {"action": "block", "message": "hookify denied"})
        self.assertEqual([call.args[0] for call in run_subcommand.call_args_list], ["observe", "governance-capture", "preflight", "hookify"])
        self.assertEqual(run_subcommand.call_args_list[0].args[1]["cwd"], "/project")

    def test_preflight_action_block_short_circuits_hookify(self):
        from unittest.mock import patch

        callback = self._callback()
        with patch.object(
            plugin, "run_subcommand", side_effect=[None, None, {"action": "block", "message": "no verify"}]
        ) as run_subcommand:
            result = callback(tool_name="Bash", args={"command": "git commit --no-verify"})
        self.assertEqual(result, {"action": "block", "message": "no verify"})
        self.assertEqual([call.args[0] for call in run_subcommand.call_args_list], ["observe", "governance-capture", "preflight"])

    def test_file_tool_runs_hookify_before_config_protection(self):
        from unittest.mock import patch

        callback = self._callback()
        with patch.object(
            plugin, "run_subcommand", side_effect=[None, None, {"decision": "block", "reason": "rule"}]
        ) as run_subcommand:
            result = callback(tool_name="write_file", args={"path": "src/a.py"})
        self.assertEqual(result, {"action": "block", "message": "rule"})
        self.assertEqual([call.args[0] for call in run_subcommand.call_args_list], ["observe", "governance-capture", "hookify"])

    def test_powershell_preflight_and_file_hookify_tool_sets(self):
        self.assertIn("pwsh", plugin._PREFLIGHT_TOOLS)
        self.assertNotIn("powershell", plugin._HOOKIFY_TOOLS)
        self.assertIn("Edit", plugin._HOOKIFY_TOOLS)
        self.assertIn("MultiEdit", plugin._HOOKIFY_TOOLS)

    def test_registers_post_tool_observer_and_skill_lifecycle(self):
        context = HookContext()
        plugin.register(context)
        self.assertIn("post_tool_call", context.hooks)
        self.assertIn("on_session_start", context.hooks)
        self.assertIn("on_session_finalize", context.hooks)

    def test_session_hooks_forward_native_lifecycle_payloads(self):
        from unittest.mock import patch

        with patch.object(plugin, "run_subcommand") as run_subcommand:
            plugin.on_session_start(session_id="s-1", cwd="/project", platform="cli")
            plugin.on_session_finalize(session_id="s-1", reason="session_boundary")
        self.assertEqual([call.args[0] for call in run_subcommand.call_args_list], ["session-hooks", "session-hooks"])
        start, finalize = [call.args[1] for call in run_subcommand.call_args_list]
        self.assertEqual(start["hook_event_name"], "on_session_start")
        self.assertEqual(start["cwd"], "/project")
        self.assertEqual(finalize["hook_event_name"], "on_session_finalize")
        self.assertEqual(finalize["reason"], "session_boundary")

    def test_post_tool_dispatch_passes_hermes_result_and_status(self):
        from unittest.mock import patch

        context = HookContext()
        plugin.register(context)
        with patch.object(plugin, "run_subcommand", return_value=None) as run_subcommand:
            plugin.on_post_tool_call(
                tool_name="skill",
                args={"skill": "demo"},
                result={"error": "failed"},
                status="error",
                error_type="tool_error",
                error_message="failed",
                cwd="/project",
                session_id="session-1",
            )
        self.assertEqual([call.args[0] for call in run_subcommand.call_args_list], ["post-observer", "lifecycle"])
        payload = run_subcommand.call_args_list[0].args[1]
        self.assertEqual(payload["result"], {"error": "failed"})
        self.assertEqual(payload["status"], "error")
        self.assertEqual(payload["error_type"], "tool_error")
        self.assertEqual(payload["error_message"], "failed")
        self.assertEqual(payload["session_id"], "session-1")


class NativePluginMigrationTests(unittest.TestCase):
    def test_shell_hook_enablement_does_not_register_config_protection(self):
        enable_source = (REPO_ROOT / "src/enable.rs").read_text(encoding="utf-8")
        self.assertNotIn('sub: "config-protection"', enable_source)

    def test_legacy_config_protection_shell_installer_is_removed(self):
        self.assertFalse((REPO_ROOT / "adapters/hermes/hooks/install-config-protection.sh").exists())

    def test_installer_can_enable_the_native_plugin_with_hermes(self):
        installer = (REPO_ROOT / "install.sh").read_text(encoding="utf-8")
        self.assertIn("--enable-plugin", installer)
        self.assertIn("hermes plugins enable foundation", installer)


if __name__ == "__main__":
    unittest.main()
