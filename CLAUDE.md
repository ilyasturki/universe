# Universe

The app is written by agents and its owner does not read the code: the tests are how behaviour is pinned, so a change is done when a test shows it working.

## Testing

A test drives the app through a real **seam**:

- the CLI on a temp profile;
- the in-process core on a sealed profile (`paths::test_env()`, the Look driver on `universe_core.Core()`);
- the UI driven by keys (ui/tests/looks.py);
- a module hook or source run as a real process against shim binaries;
- private D-Bus, PipeWire or NetworkManager services (crates/universe/src/testbus, dbusmock).

A test stays isolated only when it pins an external format, protocol or device fact, a pure algorithm with many edge cases, or a contract across files. Use the app's own modules for real: fake only what lies outside the app (stores, systemd, compositors, devices).

Assert ids, action keys and rules. Copy, card order and layout change freely, so tests leave them alone. Golden files are for files another program reads (emulator configs), never for UI copy.

One test per new behaviour, at the highest seam that reaches it. Extend an existing test before adding one.

A test over 2 s fails unless marked `@pytest.mark.slow`. A Qt warning, or an exception escaping a slot, fails the test.

The full check is `just verify` (lint and every suite; the pre-push hook runs it). To see a UI change, follow the run-ui skill (.claude/skills/run-ui/SKILL.md).
