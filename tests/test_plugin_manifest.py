from __future__ import annotations

import importlib.util
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/sync_plugin_manifest.py"
SPEC = importlib.util.spec_from_file_location("sync_plugin_manifest", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
sync = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sync)


def test_plugin_yaml_matches_implementation():
    text = sync.MANIFEST.read_text(encoding="utf-8")
    assert sync.render(text) == text


def test_render_fixes_stale_version_and_hooks():
    stale = 'version: "0.0.1"\nprovides_hooks:\n  - gone_hook\nexternal_dependencies:\n'
    fixed = sync.render(stale)
    assert f'version: "{sync.cargo_version()}"' in fixed
    assert "gone_hook" not in fixed
    assert "  - on_session_end\n" in fixed
    assert fixed.endswith("external_dependencies:\n")
