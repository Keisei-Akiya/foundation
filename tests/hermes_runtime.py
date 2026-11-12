"""Shared isolated Hermes runtime used by native-plugin integration tests."""
from __future__ import annotations

import os
import shutil
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
PLUGIN_SOURCE = REPO_ROOT / "adapters/hermes/plugin"
BINARY_PATH = Path(os.environ.get("FOUNDATION_TEST_BINARY", "/__foundation_binary_not_set__"))
HERMES_SOURCE = Path(os.environ.get("HERMES_AGENT_ROOT", Path.home() / ".hermes/hermes-agent"))
if str(HERMES_SOURCE) not in __import__("sys").path:
    __import__("sys").path.insert(0, str(HERMES_SOURCE))


def hermes_runtime_available() -> bool:
    return (HERMES_SOURCE / "hermes_cli/plugins.py").is_file()


class IsolatedHermesHome:
    """Build a temporary HERMES_HOME with the real foundation plugin and binary."""

    def __init__(self, *, enable_foundation: bool = False) -> None:
        self.enable_foundation = enable_foundation
        self._temp = tempfile.TemporaryDirectory(prefix="foundation-hermes-runtime-")
        self.root = Path(self._temp.name)
        self.home = self.root / ".hermes"
        self.plugins = self.home / "plugins"
        self.bin = self.home / "bin"

    def __enter__(self) -> "IsolatedHermesHome":
        self.plugins.mkdir(parents=True)
        self.bin.mkdir()
        if self.enable_foundation:
            (self.plugins / "foundation").symlink_to(PLUGIN_SOURCE, target_is_directory=True)
        enabled = "  enabled:\n    - foundation\n" if self.enable_foundation else "  enabled: []\n"
        (self.home / "config.yaml").write_text("plugins:\n" + enabled, encoding="utf-8")
        self._old = {key: os.environ.get(key) for key in ("HERMES_HOME", "FOUNDATION_BIN", "PYTHONPATH")}
        os.environ["HERMES_HOME"] = str(self.home)
        os.environ["FOUNDATION_BIN"] = str(self.bin / "foundation")
        hermes_python = str(HERMES_SOURCE)
        os.environ["PYTHONPATH"] = os.pathsep.join(filter(None, (hermes_python, self._old["PYTHONPATH"])))
        return self

    def __exit__(self, *_: Any) -> None:
        for key, value in self._old.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
        self._temp.cleanup()


def invoke_hook(event: str, **kwargs: Any) -> list[Any]:
    """Run Hermes' own hook dispatcher, which lazy-loads enabled plugins."""
    from hermes_cli.plugins import invoke_hook as hermes_invoke_hook

    return hermes_invoke_hook(event, **kwargs)


def registered_hooks() -> set[str]:
    """Get live event names from Hermes, not a locally maintained copy."""
    from hermes_cli.plugins import VALID_HOOKS

    return set(VALID_HOOKS)
