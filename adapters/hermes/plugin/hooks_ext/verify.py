def register(ctx, core):
    def pre_verify(**kwargs):
        import os
        payload = core.payload("pre_verify", kwargs)
        payload.update({"cwd": kwargs.get("cwd") or os.getcwd(),
                       "final_response": kwargs.get("final_response") or "",
                       "changed_paths": kwargs.get("changed_paths") or [],
                       "attempt": kwargs.get("attempt", 0)})
        if kwargs.get("attempt", 0) >= 3:
            return None
        results = []
        for name in ("stop-hookify", "format-typecheck", "check-console-log"):
            result = core.run_subcommand(name, payload, strict=False, timeout=5)
            if result and result.get("message"):
                results.append(str(result["message"]))
        return {"action": "continue", "message": "\n".join(results)} if results else None
    ctx.register_hook("pre_verify", pre_verify)
