"""Session/file first-touch factual reminder extension."""

def register(ctx, core):
    def pre(**kwargs):
        result = core.run_subcommand("gateguard", core.payload("pre_tool_call", kwargs), strict=False, timeout=5)
        if result and result.get("action") == "block":
            return result
        return None
    ctx.register_hook("pre_tool_call", pre)
