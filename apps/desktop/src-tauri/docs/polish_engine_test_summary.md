# Polish Engine Test Summary

This document records the original polish-engine test inventory. Test names and counts may change; use `cargo test --lib polish_engine` for the current result.

## Unit Test Areas

| Module | Covered behavior |
|--------|------------------|
| `common.rs` | Language-name conversion, language detection, and engine configuration |
| `traits.rs` | Engine types, request builders, result construction, display, and serialization |
| `templates.rs` | Template lookup, required fields, and unique identifiers |
| `qwen/models.rs` | Qwen model lookup, filenames, URLs, and metadata |
| `lfm/models.rs` | LFM model lookup, filenames, URLs, and metadata |
| `unified_manager.rs` | Manager creation, model routing, filenames, cache operations, and model information |

## Integration Test Areas

`tests/polish_engine_test.rs` covers:

- engine type conversion and serialization
- unified-manager initialization and model detection
- model and template availability
- request and result construction
- filename lookup and cache operations
- language-preservation instructions
- model metadata completeness

## Test Properties

The unit tests avoid network and downloaded model dependencies. Tests that run real inference belong in ignored integration suites because they require local model files.

## Commands

```bash
# Unit tests
cargo test --lib polish_engine

# Integration tests
cargo test --test polish_engine_test

# One test by name
cargo test test_language_name

# Include captured output
cargo test polish_engine -- --nocapture
```

## Locations

```text
src/polish_engine/             # Unit tests next to implementation
tests/polish_engine_test.rs    # Integration tests
docs/polish_engine_tests.md    # Test guide
```
