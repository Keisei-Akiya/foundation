from __future__ import annotations

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("foundation_plugin", ROOT / "adapters/hermes/plugin/__init__.py")
assert SPEC is not None and SPEC.loader is not None
plugin = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(plugin)


class Context:
    def __init__(self) -> None:
        self.skills: dict[str, tuple[Path, str]] = {}

    def register_hook(self, event, callback) -> None:
        pass

    def register_skill(self, name, path, description="") -> None:
        self.skills[name] = (path, description)


def test_register_does_not_duplicate_skills_registered_from_external_dir():
    ctx = Context()
    plugin.register(ctx)
    assert not ctx.skills
