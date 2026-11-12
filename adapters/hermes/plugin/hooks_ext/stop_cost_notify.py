"""Dispatch Hermes usage/final-turn hooks to the native cost and notification handlers."""


def register(ctx, core):
    def send(subcommand, event, kwargs):
        payload = core.payload(event, kwargs)
        for field in ("usage", "model", "response_model"):
            if field in kwargs:
                payload[field] = kwargs[field]
        try:
            core.run_subcommand(subcommand, payload, timeout=5)
        except Exception:
            return None

    def cost_api(**kwargs):
        return send("cost-tracker", "post_api_request", kwargs)

    def cost_turn(**kwargs):
        return send("cost-tracker", "post_llm_call", kwargs)

    def notify(**kwargs):
        return send("desktop-notify", "post_llm_call", kwargs)

    ctx.register_hook("post_api_request", cost_api)
    ctx.register_hook("post_llm_call", cost_turn)
    ctx.register_hook("post_llm_call", notify)
