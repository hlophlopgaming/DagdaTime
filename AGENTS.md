# AGENTS.md — Instructions for AI coding agents

Read `PLAN.md` first. It defines the scope, stack, architecture and milestones.

## Rules

1. **One milestone per session.** Work only on the milestone you were given. Do not start the next one.
2. **Tests first.** Write tests for the acceptance criteria, then the implementation.
3. **Platform code only behind traits** (`WindowWatcher`, `IdleWatcher`, …). Business logic must be testable with `MockWatcher` on any OS.
4. **No schema changes without a migration.** Never edit an existing migration; add a new one.
5. **Timestamps:** unix milliseconds, UTC, everywhere in storage and API. Convert to local time only in the UI.
6. **Privacy:** no network calls except to `localhost`. No telemetry.
7. **Before finishing**, run and show results of:
   ```
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```
8. **Record decisions** that affect architecture in `docs/decisions.md` (date, decision, reason), so future sessions don't redo them.
9. You usually cannot run code on Windows/macOS/Wayland. For platform-specific code, say explicitly what needs manual verification and how.

## Prompt template

> Implement milestone **M_X** from PLAN.md.
> Acceptance criteria: [copy from PLAN.md].
> Write tests first, then code. Stay within this milestone.
> At the end run fmt, clippy and tests and show the output, and list anything that needs manual verification on a specific OS.
