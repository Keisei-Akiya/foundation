"""MCP failure tracking and preflight guard extension."""

def register(ctx, core):
    def pre(**kwargs):
        payload = core.payload("pre_tool_call", kwargs)
        result = core.run_subcommand("mcp-health", payload, strict=False, timeout=5)
        if result and result.get("action") == "block":
            return result
        return None
    def post(**kwargs):
        payload = core.payload("post_tool_call", kwargs)
        core.run_subcommand("mcp-health", payload, strict=False, timeout=5)
    ctx.register_hook("pre_tool_call", pre)
    ctx.register_hook("post_tool_call", post)
