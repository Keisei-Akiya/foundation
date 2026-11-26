from __future__ import annotations

import importlib.util
from pathlib import Path
from unittest.mock import Mock

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("foundation_plugin", ROOT / "adapters/hermes/plugin/__init__.py")
assert SPEC is not None and SPEC.loader is not None
plugin = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(plugin)


def test_register_does_not_duplicate_skills_registered_from_external_dir():
    ctx = Mock()
    plugin.register(ctx)
    ctx.register_skill.assert_not_called()
