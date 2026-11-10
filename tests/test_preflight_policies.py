"""Positive and negative tests for portable pre-call policies.

Run by the parent after the concurrent worker's test window: python -m unittest tests.test_preflight_policies
"""
import importlib.util
import json
import subprocess
import sys
import unittest
from pathlib import Path

MODULE = Path(__file__).resolve().parents[1] / "adapters/hermes/hooks/preflight_policies.py"
spec = importlib.util.spec_from_file_location("preflight_policies", MODULE)
assert spec and spec.loader
policies = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = policies
spec.loader.exec_module(policies)


class PreflightPoliciesTests(unittest.TestCase):
    def test_narrow_no_verify_guard_blocks_literal_git_commit_but_allows_other_commands(self):
        blocked = policies.evaluate({"tool_name": "terminal", "args": {"command": "git commit -m 'ok' --no-verify"}})
        allowed = policies.evaluate({"tool_name": "terminal", "args": {"command": "git status --short"}})
        self.assertEqual(blocked.directive()["action"], "block")
        self.assertIsNone(allowed.directive())

    def test_no_verify_does_not_apply_to_other_tools(self):
        result = policies.evaluate({"tool_name": "write_file", "args": {"path": "notes.md", "content": "git commit --no-verify"}})
        self.assertIsNone(result.directive())

    def test_powerShell_destructive_command_is_blocked_and_read_only_allowed(self):
        blocked = policies.evaluate({"tool_name": "PowerShell", "args": {"command": "Remove-Item C:\\data -Recurse -Force"}})
        allowed = policies.evaluate({"tool_name": "PowerShell", "args": {"command": "Get-ChildItem"}})
        self.assertEqual(blocked.action, "block")
        self.assertIsNone(allowed.directive())

    def test_advisory_warnings_are_disabled_when_hermes_cannot_surface_them(self):
        result = policies.evaluate({"tool_name": "write_file", "args": {"path": "scratch/TODO.md", "content": "x"}})
        self.assertIsNone(result.directive())

    def test_count_and_governance_features_are_disabled_without_durable_state_or_sink(self):
        event = {"tool_name": "write_file", "task_id": "s1", "args": {"path": ".env"}}
        result = policies.evaluate(event, env={"COMPACT_THRESHOLD": "1", "ECC_GOVERNANCE_CAPTURE": "1"})
        self.assertIsNone(result.directive())

    def test_sensitive_path_is_not_misrepresented_as_stateful_gate(self):
        event = {"tool_name": "write_file", "args": {"path": ".env", "content": "x"}}
        self.assertIsNone(policies.evaluate(event, state={}).directive())
        self.assertIsNone(policies.evaluate(event, state={"touched_paths": {".env"}}).directive())
        trivial = {"tool_name": "write_file", "args": {"path": ".env", "content": "x", "trivial": True}}
        self.assertIsNone(policies.evaluate(trivial, state={}).directive())

    def test_sensitive_gate_does_not_block_normal_paths(self):
        result = policies.evaluate({"tool_name": "write_file", "args": {"path": "src/main.py", "content": "x"}})
        self.assertIsNone(result.directive())

    def test_strict_advisories_are_disabled_until_hermes_can_surface_them(self):
        event = {"tool_name": "terminal", "args": {"command": "git push origin main"}}
        self.assertIsNone(policies.evaluate(event, profile="strict", env={}).directive())
        self.assertIsNone(policies.evaluate(event, profile="standard", env={}).directive())

    def test_stdin_runner_outputs_only_valid_block_directive(self):
        payload = json.dumps({"hook_event_name": "pre_tool_call", "tool_name": "terminal", "tool_input": {"command": "git commit --no-verify"}, "session_id": "s1", "cwd": "/workspace", "extra": {}})
        run = subprocess.run([sys.executable, str(MODULE)], input=payload, text=True, capture_output=True, check=False)
        self.assertEqual(run.returncode, 0)
        self.assertEqual(json.loads(run.stdout), {"action": "block", "message": "Preflight block: detected literal git commit --no-verify; shell syntax and git hook-redirect variants are not covered."})

    def test_hermes_run_once_parser_blocks_across_fresh_processes(self):
        workspaces = sorted((Path.home() / ".hermes/installs").glob("*/environments/*/workspace"))
        self.assertTrue(workspaces, "Hermes source workspace is required for integration test")
        workspace = workspaces[-1]
        script = (
            "import json, sys; "
            f"sys.path.insert(0, {str(workspace)!r}); "
            "from agent.shell_hooks import ShellHookSpec, run_once; "
            f"spec=ShellHookSpec(event='pre_tool_call', command={sys.executable + ' ' + str(MODULE)!r}); "
            "result=run_once(spec, {'tool_name':'terminal','args':{'command':'git commit --no-verify'},'session_id':'same-session'}); "
            "print(json.dumps(result['parsed']))"
        )
        expected = {"action": "block", "message": "Preflight block: detected literal git commit --no-verify; shell syntax and git hook-redirect variants are not covered."}
        for _ in range(3):
            run = subprocess.run([sys.executable, "-c", script], text=True, capture_output=True, check=False)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(json.loads(run.stdout), expected)

    def test_hermes_run_once_does_not_surface_pre_tool_context(self):
        workspaces = sorted((Path.home() / ".hermes/installs").glob("*/environments/*/workspace"))
        self.assertTrue(workspaces, "Hermes source workspace is required for integration test")
        workspace = workspaces[-1]
        script = (
            "import json, sys; "
            f"sys.path.insert(0, {str(workspace)!r}); "
            "from agent.shell_hooks import ShellHookSpec, run_once; "
            f"spec=ShellHookSpec(event='pre_tool_call', command={sys.executable + ' ' + str(MODULE)!r}); "
            "result=run_once(spec, {'tool_name':'write_file','args':{'path':'scratch/TODO.md'},'session_id':'s'}); "
            "print(json.dumps(result['parsed']))"
        )
        run = subprocess.run([sys.executable, "-c", script], text=True, capture_output=True, check=False)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIsNone(json.loads(run.stdout))

    def test_malformed_and_oversized_input_do_not_emit_directives(self):
        for payload in ("{bad", "x" * (1024 * 1024 + 1)):
            run = subprocess.run([sys.executable, str(MODULE)], input=payload, text=True, capture_output=True, check=False)
            self.assertEqual(run.returncode, 0)
            self.assertEqual(run.stdout, "")


if __name__ == "__main__":
    unittest.main()
