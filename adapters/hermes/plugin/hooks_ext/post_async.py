"""Dispatch post-tool observers on a bounded background worker."""
from __future__ import annotations

import threading
from typing import Any


def register(ctx: Any, core: Any) -> None:
    def on_post_tool_call(**kwargs: Any) -> None:
        payload = core.payload("post_tool_call", kwargs)

        def dispatch() -> None:
            try:
                core.run_subcommand("post-async", payload, strict=False, timeout=5)
            except Exception as error:  # noqa: BLE001 - observer is fail-open
                core.logger.warning("foundation post-async failed: %s", error)

        # The callback must not await observer work. The daemon worker lets
        # Hermes shut down without joining it; work still in flight at shutdown
        # may be lost. The helper's timeout bounds each child subprocess.
        threading.Thread(
            target=dispatch,
            name="foundation-post-async",
            daemon=True,
        ).start()

    ctx.register_hook("post_tool_call", on_post_tool_call)
