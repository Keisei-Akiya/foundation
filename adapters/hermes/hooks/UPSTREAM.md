# Upstream attribution

The following files are verbatim copies from ECC at commit
`d2ef2680594f9a17526a6e1a2e18b3d242c5d17b`:

- `config-protection.py` — upstream `hooks/hermes/config-protection.py` (ECC hook #7).
- `check-console-log.py` — upstream `hooks/hermes/check-console-log.py` (ECC hook #22).

Copyright (c) 2026 Affaan Mustafa. Both files are distributed under the MIT License.
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

`config-protection.py` is a `pre_tool_call` guard. It emits the Hermes
`{"decision":"block","reason":"..."}` response only for existing protected
config files and recognized edit tools.

`check-console-log.py` is an observer intended for `post_tool_call`. It inspects
the edited file on disk, writes warnings to stderr, and never blocks. It is not a
`pre_verify` hook and does not consume `changed_paths`; wiring it only to
`pre_verify` would not make this original script fire. This repository copies
the originals only; registration/configuration is outside this task's file
ownership boundary.
