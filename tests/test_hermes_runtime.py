from __future__ import annotations

import os

import pytest

pytest = pytest.importorskip("pytest")

from hermes_runtime import IsolatedHermesHome, hermes_runtime_available, invoke_hook, registered_hooks

if not hermes_runtime_available():
    pytest.skip("Hermes source tree is unavailable", allow_module_level=True)


def test_foundation_plugin_is_loaded_by_hermes_and_registers_its_hooks():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        from hermes_cli.plugins import get_plugin_manager

        # Hermes discovers user plugins lazily only when its own dispatcher is invoked.
        invoke_hook("pre_tool_call", tool_name="read_file", args={"path": "README.md"})
        manager = get_plugin_manager()
        assert "foundation" in manager._plugins
        callbacks = manager.iter_hook_callbacks("pre_tool_call")
        assert callbacks
        assert any(callback.__module__.startswith("hermes_plugins.foundation") for callback in callbacks)
        assert (runtime.plugins / "foundation").is_symlink()
        assert os.environ["HERMES_HOME"] == str(runtime.home)


@pytest.mark.parametrize("event", sorted(registered_hooks()))
def test_registered_hermes_callback_is_dispatched_for_every_valid_hook(event: str):
    with IsolatedHermesHome() as runtime:
        probe = runtime.plugins / "runtime-probe"
        probe.mkdir()
        (probe / "plugin.yaml").write_text(
            "name: runtime-probe\nversion: '1'\nkind: standalone\nprovides_hooks:\n  - "
            + event
            + "\n",
            encoding="utf-8",
        )
        (probe / "__init__.py").write_text(
            "def register(ctx):\n"
            f"    ctx.register_hook({event!r}, lambda **kwargs: {{'called': {event!r}}})\n",
            encoding="utf-8",
        )
        config = runtime.home / "config.yaml"
        config.write_text(
            "plugins:\n  enabled:\n    - foundation\n    - runtime-probe\n", encoding="utf-8"
        )
        results = invoke_hook(event, runtime_test=True)
        assert {"called": event} in results


def test_foundation_unregistered_hook_smoke_coverage_slots():
    """Known future hook names remain visible while Hermes adds them over time."""
    currently_registered = registered_hooks()
    foundation_declared = {"pre_tool_call", "post_tool_call", "on_session_start", "on_session_finalize"}
    assert foundation_declared <= currently_registered
    # All other live Hermes hook names are exercised by the parametrized dispatcher test above.
    assert currently_registered - foundation_declared
