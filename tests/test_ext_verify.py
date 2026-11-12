from __future__ import annotations

from hermes_runtime import IsolatedHermesHome, invoke_hook


def test_verify_extension_returns_continue_for_findings():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        binary = runtime.bin / "foundation"
        binary.write_text("#!/bin/sh\ncase \"$1\" in check-console-log) echo '{\"action\":\"continue\",\"message\":\"console.log found\"}';; esac\n", encoding="utf-8")
        binary.chmod(0o755)
        results = invoke_hook("pre_verify", changed_paths=["src/a.ts"], final_response="done", attempt=0, cwd=str(runtime.root))
        assert any(isinstance(item, dict) and item.get("action") == "continue" for item in results)
        capped = invoke_hook("pre_verify", changed_paths=["src/a.ts"], final_response="done", attempt=3, cwd=str(runtime.root))
        assert not any(isinstance(item, dict) and item.get("action") == "continue" for item in capped)
