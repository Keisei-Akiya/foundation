"""Integration checks against Hermes' real shell-hook registration and execution path."""
from __future__ import annotations

import importlib.util
import os
import shlex
import sys
from pathlib import Path

import pytest
from ruamel.yaml import YAML

FOUNDATION = Path(__file__).resolve().parents[1]
HOOKS = FOUNDATION / "adapters" / "hermes" / "hooks"


def _find_hermes_agent() -> Path | None:
    """Resolve Hermes source explicitly or from standard local checkout locations."""
    configured = os.environ.get("HERMES_AGENT_DIR")
    if configured:
        candidate = Path(configured).expanduser().resolve()
        return candidate if (candidate / "agent" / "shell_hooks.py").is_file() else None

    candidates = [Path.home() / ".hermes" / "hermes-agent"]
    candidates.extend(parent / "hermes-agent" for parent in Path(__file__).resolve().parents)
    for candidate in candidates:
        candidate = candidate.resolve()
        if (candidate / "agent" / "shell_hooks.py").is_file():
            return candidate
    return None


class Manager:
    def __init__(self):
        self._hooks = {}


def _hermes_shell_hooks(monkeypatch, tmp_path):
    hermes = _find_hermes_agent()
    if hermes is None:
        pytest.skip(
            "Hermes source checkout not found; set HERMES_AGENT_DIR to its root "
            "(containing agent/shell_hooks.py) to run these integration checks"
        )
    assert hermes is not None
    monkeypatch.syspath_prepend(str(hermes))
    module_path = hermes / "agent" / "shell_hooks.py"
    spec = importlib.util.spec_from_file_location("agent.shell_hooks", module_path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    home = tmp_path / "isolated-hermes-home"
    home.mkdir()
    monkeypatch.setenv("HERMES_HOME", str(home))
    manager = Manager()
    import hermes_cli.plugins
    monkeypatch.setattr(hermes_cli.plugins, "get_plugin_manager", lambda: manager)
    return module, manager, home


def test_sample_command_is_argv_safe_and_resolves(monkeypatch, tmp_path):
    """Commands in the public example remain parseable when paths contain spaces."""
    example = (HOOKS / "hooks.example.yaml").read_text()
    assert "FOUNDATION_ROOT" in example
    command = f'python3 "{HOOKS / "preflight_policies.py"}"'
    argv = shlex.split(command)
    assert Path(argv[1]).is_file()
    assert argv[0] == "python3"


def test_real_registration_and_block_allow_firing_in_isolated_home(monkeypatch, tmp_path):
    module, manager, home = _hermes_shell_hooks(monkeypatch, tmp_path)
    command = f'{shlex.quote(sys.executable)} {shlex.quote(str(HOOKS / "preflight_policies.py"))}'
    cfg = {
        "hooks": {
            "pre_tool_call": [{
                "command": command,
                "matcher": "^(terminal|bash|Bash)$",
                "timeout": 5,
                "fail_closed": False,
            }]
        }
    }
    registered = module.register_from_config(cfg, accept_hooks=True)
    assert len(registered) == 1
    assert len(manager._hooks["pre_tool_call"]) == 1
    assert (home / module.ALLOWLIST_FILENAME).is_file()

    blocked = manager._hooks["pre_tool_call"][0](
        tool_name="terminal", args={"command": "git commit --no-verify -m test"}
    )
    allowed = manager._hooks["pre_tool_call"][0](
        tool_name="terminal", args={"command": "git status --short"}
    )
    assert blocked and blocked["action"] == "block"
    assert allowed is None

    spec = registered[0]
    run_once = module.run_once(spec, {"tool_name": "terminal", "args": {"command": "git commit --no-verify"}})
    assert run_once["parsed"]["action"] == "block"


def test_real_runner_config_protection_preflight_allows_and_blocks_edit_file(monkeypatch, tmp_path):
    module, manager, _home = _hermes_shell_hooks(monkeypatch, tmp_path)
    config = tmp_path / ".prettierrc"
    config.write_text("{}\\n")
    wrapper = HOOKS / "hermes-tool-compat.py"
    command = f'{shlex.quote(sys.executable)} {shlex.quote(str(wrapper))} config-protection.py'
    registered = module.register_from_config({"hooks": {"pre_tool_call": [{
        "command": command, "matcher": "^(write_file|patch|edit_file)$",
        "timeout": 5, "fail_closed": False,
    }]}}, accept_hooks=True)
    assert len(registered) == 1
    callback = manager._hooks["pre_tool_call"][0]
    blocked = callback(tool_name="edit_file", args={"path": str(config), "new_string": "bad"})
    allowed = callback(tool_name="edit_file", args={"path": str(tmp_path / "app.py"), "content": "ok"})
    assert blocked and blocked["action"] == "block"
    assert "BLOCKED" in blocked["message"]
    assert allowed is None


def test_real_runner_console_checker_uses_file_event_tool_matcher(monkeypatch, tmp_path, caplog):
    module, manager, _home = _hermes_shell_hooks(monkeypatch, tmp_path)
    source = tmp_path / "app.ts"
    source.write_text("console.log('debug');\\n")
    wrapper = HOOKS / "hermes-tool-compat.py"
    command = f'{shlex.quote(sys.executable)} {shlex.quote(str(wrapper))} check-console-log.py'
    registered = module.register_from_config({"hooks": {"post_tool_call": [{
        "command": command, "matcher": "^(write_file|patch|edit_file)$",
        "timeout": 5, "fail_closed": False,
    }]}}, accept_hooks=True)
    assert len(registered) == 1
    callback = manager._hooks["post_tool_call"][0]
    caplog.set_level("DEBUG")
    callback(tool_name="edit_file", args={"path": str(source), "new_string": "debug"})
    # Shell-hook stderr is captured at debug level, not surfaced to the user.
    assert "console.log" in caplog.text
    assert source.read_text() == "console.log('debug');\\n"


def test_real_runner_hookify_file_rule_matches_edit_file_and_allows_other_path(monkeypatch, tmp_path):
    monkeypatch.chdir(tmp_path)
    rule_dir = tmp_path / ".claude"
    rule_dir.mkdir()
    (rule_dir / "hookify.secret.local.md").write_text(
        "---\nevent: file\naction: block\npattern: \\.env$\n---\nNo secret edits\n",
        encoding="utf-8",
    )
    module, manager, _home = _hermes_shell_hooks(monkeypatch, tmp_path)
    script = HOOKS / "hookify_runtime.py"
    command = f'{shlex.quote(sys.executable)} {shlex.quote(str(script))}'
    registered = module.register_from_config({"hooks": {"pre_tool_call": [{
        "command": command, "matcher": "^(read_file|write_file|patch|edit_file|apply_patch)$",
        "timeout": 5, "fail_closed": False,
    }]}}, accept_hooks=True)
    assert len(registered) == 1
    callback = manager._hooks["pre_tool_call"][0]
    blocked = callback(tool_name="edit_file", args={"path": ".env", "new_string": "secret"})
    allowed = callback(tool_name="edit_file", args={"path": "notes.txt", "new_string": "safe"})
    assert blocked and blocked["action"] == "block"
    assert "No secret edits" in blocked["message"]
    assert allowed is None


def test_real_runner_hookify_bash_sample_matcher_alias_and_malformed_rule_fail_open(monkeypatch, tmp_path):
    monkeypatch.chdir(tmp_path)
    rule_dir = tmp_path / ".claude"
    rule_dir.mkdir()
    (rule_dir / "hookify.bash.local.md").write_text(
        "---\nevent: bash\naction: block\npattern: npm\\s+publish\n---\nNo publish\n",
        encoding="utf-8",
    )
    (rule_dir / "hookify.broken.local.md").write_text(
        "---\nevent: bash\naction: block\npattern: [\n---\nMalformed\n",
        encoding="utf-8",
    )
    module, manager, _home = _hermes_shell_hooks(monkeypatch, tmp_path)
    command = f'{shlex.quote(sys.executable)} {shlex.quote(str(HOOKS / "hookify_runtime.py"))}'
    registered = module.register_from_config({"hooks": {"pre_tool_call": [{
        "command": command, "matcher": "^(terminal|bash|Bash|read_file|write_file|patch|edit_file|apply_patch)$",
        "timeout": 5, "fail_closed": False,
    }]}}, accept_hooks=True)
    assert len(registered) == 1
    callback = manager._hooks["pre_tool_call"][0]
    blocked = callback(tool_name="bash", args={"command": "npm publish"})
    allowed = callback(tool_name="bash", args={"command": "git status"})
    assert blocked and blocked["action"] == "block"
    assert "No publish" in blocked["message"]
    assert allowed is None


def test_sample_yaml_is_valid_yaml(monkeypatch, tmp_path):
    data = YAML().load((HOOKS / "hooks.example.yaml").read_text())
    assert data is None  # all opt-ins remain commented out by default
