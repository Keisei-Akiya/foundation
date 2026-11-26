from __future__ import annotations

import importlib.util
from pathlib import Path
from unittest.mock import Mock

ROOT = Path(__file__).resolve().parents[1]
PLUGIN_DIR = ROOT / "adapters/hermes/plugin"
SPEC = importlib.util.spec_from_file_location("foundation_plugin", PLUGIN_DIR / "__init__.py")
assert SPEC is not None and SPEC.loader is not None
plugin = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(plugin)


def _declared_hooks() -> set[str]:
    hooks: set[str] = set()
    in_block = False
    for line in (PLUGIN_DIR / "plugin.yaml").read_text(encoding="utf-8").splitlines():
        if line.startswith("provides_hooks:"):
            in_block = True
        elif in_block and line.startswith("  - "):
            hooks.add(line[4:].strip())
        elif in_block:
            break
    return hooks


def test_provides_hooks_matches_registered_hooks():
    ctx = Mock()
    plugin.register(ctx)
    registered = {call.args[0] for call in ctx.register_hook.call_args_list}
    assert _declared_hooks() == registered
