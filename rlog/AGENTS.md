# AGENTS.md

Guidelines for the shared `rlog` crate.

## Scope

This directory is a shared Rust logging crate used by multiple apps.
Do not treat changes here as `kfk`-specific.

When modifying this crate:
- Preserve existing public APIs unless the user explicitly asks for a breaking change.
- Prefer adding fallible `try_*` APIs before changing panic-based APIs.
- Keep behavior compatible with existing callers in `apps/kfk`, `apps/dbm`, and other consumers.
- Add or update focused tests for rolling, retention, and initialization behavior when practical.

## Rust Style

- Keep changes small and local.
- Match the existing direct style.
- Avoid adding new dependencies unless they remove real complexity.
- Prefer `Result` for new initialization/configuration APIs.
- Do not silently swallow filesystem/configuration errors in new APIs.

## Logging Behavior

- Preserve the existing default pattern unless explicitly changed.
- Be careful with rolling and retention behavior because it affects production log retention.
- Avoid hardcoded user/group ownership in new code unless documented as an operational requirement.

## Verification

Do not run `cargo fmt` for this crate unless the user explicitly asks for it.
Keep formatting changes limited to lines touched for the requested behavior.

Before finishing changes, run the narrowest relevant checks, such as:

```sh
cargo test -p rlog
cargo check -p rlog
```
