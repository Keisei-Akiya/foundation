from __future__ import annotations

import json
import os
from pathlib import Path
import shutil

import pytest

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, hermes_runtime_available, invoke_hook

if not hermes_runtime_available():
    pytest.skip("Hermes source tree is unavailable", allow_module_level=True)


def _install_binary(runtime: IsolatedHermesHome) -> None:
    assert BINARY_PATH.is_file(), "build foundation before running runtime pre-tool tests"
    destination = runtime.bin / "foundation"
    shutil.copy2(BINARY_PATH, destination)
    destination.chmod(0o755)


def _enable_foundation(runtime: IsolatedHermesHome) -> None:
    config = runtime.home / "config.yaml"
    config.write_text("plugins:\n  enabled:\n    - foundation\n", encoding="utf-8")


def test_pre_tool_call_dispatches_preflight_through_hermes_and_blocks_no_verify():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_binary(runtime)

        results = invoke_hook(
            "pre_tool_call",
            tool_name="terminal",
            args={"command": "git commit --no-verify -m test"},
            cwd=str(runtime.root),
        )

        assert any(
            isinstance(result, dict)
            and result.get("action") == "block"
            and "no-verify" in result.get("message", "")
            for result in results
        )


def test_pre_tool_call_dispatches_project_hookify_rule_through_hermes():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_binary(runtime)
        rules = runtime.root / ".claude"
        rules.mkdir()
        (rules / "hookify.no-rm.local.md").write_text(
            "---\nname: no-rm\nevent: bash\naction: block\npattern: rm -rf\n---\n"
            "Do not recursively delete files.\n",
            encoding="utf-8",
        )

        results = invoke_hook(
            "pre_tool_call",
            tool_name="terminal",
            args={"command": "rm -rf ./data"},
            cwd=str(runtime.root),
        )

        assert any(
            isinstance(result, dict)
            and result.get("action") == "block"
            and "no-rm" in result.get("message", "")
            for result in results
        )


def test_pre_tool_call_dispatches_config_protection_through_hermes():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_binary(runtime)
        protected = runtime.root / "eslint.config.js"
        protected.write_text("export default {};\n", encoding="utf-8")

        results = invoke_hook(
            "pre_tool_call",
            tool_name="write_file",
            args={"path": str(protected), "content": "weakened"},
            cwd=str(runtime.root),
        )

        assert any(
            isinstance(result, dict)
            and result.get("action") == "block"
            and "linter/formatter" in result.get("message", "")
            for result in results
        )


def test_pre_tool_call_dispatches_continuous_learning_observer_through_hermes():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_binary(runtime)
        settings = runtime.home / "continuous-learning-v2"
        settings.mkdir()
        (settings / "config.json").write_text(
            json.dumps({"observer": {"enabled": True}}), encoding="utf-8"
        )

        invoke_hook(
            "pre_tool_call",
            tool_name="read_file",
            args={"path": "README.md"},
            cwd=str(runtime.root),
        )

        observations = runtime.home / "plugin-data/ecc-hooks/continuous-learning-observations.jsonl"
        records = [json.loads(line) for line in observations.read_text(encoding="utf-8").splitlines()]
        assert {"event": "pre:observe:continuous-learning", "tool_name": "read_file"} in records


def test_pre_tool_call_dispatches_governance_capture_opt_in_through_hermes():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_binary(runtime)
        previous = os.environ.get("ECC_GOVERNANCE_CAPTURE")
        os.environ["ECC_GOVERNANCE_CAPTURE"] = "1"
        try:
            # Governance capture is best-effort stderr telemetry; exercise its opted-in
            # classification via the actual Hermes callback, and assert it does not block.
            results = invoke_hook(
                "pre_tool_call",
                tool_name="terminal",
                args={"command": "git push --force origin feature"},
                cwd=str(runtime.root),
            )
        finally:
            if previous is None:
                os.environ.pop("ECC_GOVERNANCE_CAPTURE", None)
            else:
                os.environ["ECC_GOVERNANCE_CAPTURE"] = previous

        assert not any(
            isinstance(result, dict) and result.get("action") == "block" for result in results
        )


def test_pre_tool_call_dispatcher_runs_multiple_guards_in_one_hermes_invocation():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_binary(runtime)
        rules = runtime.root / ".claude"
        rules.mkdir()
        (rules / "hookify.no-rm.local.md").write_text(
            "---\nname: no-rm\nevent: bash\naction: block\npattern: rm -rf\n---\n"
            "Denied by project rule.\n",
            encoding="utf-8",
        )

        results = invoke_hook(
            "pre_tool_call",
            tool_name="terminal",
            args={"command": "rm -rf ./data"},
            cwd=str(runtime.root),
        )

        assert any(
            isinstance(result, dict)
            and result.get("action") == "block"
            and "no-rm" in result.get("message", "")
            for result in results
        )
