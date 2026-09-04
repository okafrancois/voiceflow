# ADR-006: Original target delivery

**Date**: 2026-09-04
**Status**: Accepted

## Decision

Snapshot original-target delivery settings and the macOS target when recording
starts. Foreground mode activates and verifies the source application before
using the existing injector; background mode writes only through the captured
Accessibility element and never falls back to current focus.

## Rationale

Simulated keyboard and paste events always go to current focus. Delayed
transcription makes that focus unsafe as an implicit destination after the user
switches applications. Foreground activation works with more applications, but
interrupts the user. Accessibility insertion avoids that interruption, but
many games and custom controls do not expose a writable field.

An explicit mode preserves both trade-offs. Refusing current-focus fallback in
background mode prevents private dictated text from appearing in an unrelated
application.

## Alternatives considered

- Always reactivate the original application: reliable, but prevents users from
  continuing work elsewhere without interruption.
- Always use Accessibility: cannot serve applications with custom or protected
  input fields.
- Capture only the application identifier: insufficient for background delivery
  to a specific field.
- Fall back to current focus after a target error: rejected because it can place
  text in the wrong application.

## Consequences

- Targeted delivery is opt-in and currently available only on macOS.
- Recording sessions keep an immutable enablement and mode snapshot.
- Background success records `inserted_accessibility`; failure preserves history
  and records `failed`.
- Direct polish streaming is suppressed for targeted sessions.
- Current-focus correction observation is skipped after background insertion.
- Native compatibility still requires manual testing against representative
  third-party applications.

