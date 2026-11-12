# Upstream attribution

The behavior of the following hooks was derived from ECC files at commit
`d2ef2680594f9a17526a6e1a2e18b3d242c5d17b`. The upstream Python originals have
been removed; the behavior is now implemented in the Rust `foundation` binary:

- config-protection — derived from ECC's Hermes config-protection hook (ECC hook #7). Implemented by the `config-protection` subcommand.
- check-console-log — derived from ECC's Hermes check-console-log hook (ECC hook #22). Implemented as the console-log check in the `post-observer` subcommand.
- post:dispatcher:sync — partial native migration. Hermes `post_tool_call` dispatches to Rust `post-observer` and `lifecycle`; no arbitrary ECC child-hook registry or per-child controls are implemented.
- post:skill:track — partial native migration. Rust `lifecycle` persists minimal Skill invocation metadata and success/failure outcome when `ECC_HERMES_SKILL_TRACK=1`.

Copyright (c) 2026 Affaan Mustafa. The derived upstream hooks are covered by the MIT License.
The upstream `LICENSE` is at the ECC repository root. The required copyright and
permission notice is reproduced here:

> MIT License
>
> Copyright (c) 2026 Affaan Mustafa
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## Hermes event applicability

`config-protection` is a `pre_tool_call` guard. The `config-protection` subcommand
emits the Hermes `{"decision":"block","reason":"..."}` response only for existing
protected config files and recognized edit tools. New files and other tools are
allowed, and malformed input fails open.

`post-observer` is a `post_tool_call` observer, and its console-log check is the
upstream check-console-log behavior. It inspects the edited file on disk, writes
warnings to stderr, and never blocks. It also reports literal black/white colors
and generic font families. It is not a `pre_verify` hook and does not consume
`changed_paths`; wiring it only to `pre_verify` would not make it fire.

The native plugin registers `post_tool_call` and forwards Hermes result/status fields. Runtime dispatch in a live Hermes gateway has not been tested. The lifecycle recorder remains opt-in through `ECC_HERMES_SKILL_TRACK=1` and requires `HERMES_HOME`.
