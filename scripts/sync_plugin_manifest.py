#!/usr/bin/env python3
"""plugin.yaml の version と provides_hooks を実装から再生成する。

version は Cargo.toml、provides_hooks は register() が呼ぶ register_hook を正とする。
引数なしで書き換え、--check は差分があれば 1 を返す。
"""
from __future__ import annotations

import importlib.util
import re
import sys
from pathlib import Path
from unittest.mock import Mock

ROOT = Path(__file__).resolve().parents[1]
PLUGIN_DIR = ROOT / "adapters/hermes/plugin"
MANIFEST = PLUGIN_DIR / "plugin.yaml"


def cargo_version() -> str:
    m = re.search(r'^version = "(.*)"$', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    assert m, "Cargo.toml に version が無い"
    return m[1]


def registered_hooks() -> list[str]:
    spec = importlib.util.spec_from_file_location("foundation_plugin", PLUGIN_DIR / "__init__.py")
    assert spec and spec.loader
    plugin = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(plugin)
    ctx = Mock()
    plugin.register(ctx)
    return list(dict.fromkeys(c.args[0] for c in ctx.register_hook.call_args_list))


def render(text: str) -> str:
    text = re.sub(r'^version: ".*"$', f'version: "{cargo_version()}"', text, count=1, flags=re.M)
    block = "provides_hooks:\n" + "".join(f"  - {h}\n" for h in registered_hooks())
    return re.sub(r"^provides_hooks:\n(?:  - .*\n)*", block, text, count=1, flags=re.M)


def main() -> int:
    old = MANIFEST.read_text(encoding="utf-8")
    new = render(old)
    if new == old:
        return 0
    if "--check" in sys.argv[1:]:
        print("plugin.yaml が実装とずれている。scripts/sync_plugin_manifest.py を実行する。", file=sys.stderr)
        return 1
    MANIFEST.write_text(new, encoding="utf-8")
    print("plugin.yaml を更新した")
    return 0


if __name__ == "__main__":
    sys.exit(main())
