# ADR-003: Dual-Layer Text Injection

**Date**: 2025-10
**Status**: Accepted

## Decision

Use two injection strategies based on text shape and length:
- **Layer 0**: Keyboard simulation for short, single-line text up to 400 characters
- **Layer 2**: Transactional clipboard paste for multiline or longer text, and as the keyboard failure fallback

## Rationale

Keyboard simulation loses characters on long input due to event queue limitations. Clipboard paste is reliable but modifies clipboard state. The dual approach balances reliability with user experience.

## Alternatives Considered

- Always clipboard — breaks user clipboard state
- Always keyboard — corrupts long text
- Chunked keyboard with delays — complex, fragile, still unreliable for very long text

## Consequences

- Short text injected without clipboard modification
- Long and multiline text temporarily uses the clipboard.
- macOS restores every pasteboard item and Windows restores prior text content.
- Delivery failures propagate to the caller instead of being logged as success.
- The text length threshold is configurable at compile time.
- macOS sends `Cmd+V`; Windows sends `Ctrl+V` and releases common modifiers after
  the attempt.
