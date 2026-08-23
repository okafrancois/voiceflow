# Polish Engine Tests

## Test Structure

Polish-engine tests use two layers:

1. Unit tests live in `#[cfg(test)]` modules beside the implementation.
2. Integration tests live in `tests/polish_engine_test.rs` and exercise the public API across modules.

Real model inference is kept in ignored integration tests because it requires downloaded model files.

## Unit Test Coverage

### `common.rs`

- known and unknown language-code conversion
- empty, single-language, and mixed-language detection
- engine configuration construction

### `traits.rs`

- engine type conversion, display, parsing, and serialization
- polish request construction
- polish result construction and metrics

### `templates.rs`

- template lookup by identifier
- complete and valid template fields
- unique template identifiers

### Model Definitions

The Qwen and LFM model tests cover lookup by identifier and filename, download URLs, model-family recognition, and required metadata.

### `unified_manager.rs`

- manager construction
- model-to-engine routing
- model filename lookup
- cache clearing
- model information
- invalid model-path handling

## Integration Coverage

`tests/polish_engine_test.rs` checks engine type conversion, manager initialization, model detection, template availability, request and result builders, cache operations, language-preservation instructions, metadata completeness, and serialization.

## Commands

```bash
# Unit tests
cargo test --lib polish_engine

# Integration tests
cargo test --test polish_engine_test

# All tests matching the module name
cargo test polish_engine

# One test
cargo test test_language_name

# Captured output
cargo test polish_engine -- --nocapture
```

## Adding Tests

- Keep pure logic tests beside the implementation.
- Put cross-module behavior in the integration suite.
- Mark real inference tests as ignored and document the required model fixture.
- Verify success and failure paths without network access where possible.
