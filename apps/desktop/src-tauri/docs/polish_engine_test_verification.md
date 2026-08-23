# Polish Engine Test Verification

This file is a historical verification note. Do not treat its old test counts or compile state as current evidence.

## Previous Blocker

An earlier test run was blocked by an unrelated constructor mismatch in `src/stt_engine/unified_manager.rs`. `WhisperEngine::new` returned a `Result` and expected borrowed arguments, while the caller passed owned values without handling the result.

The intended construction pattern was:

```rust
let engine = WhisperEngine::new(&temp_dir, "tiny")?;
EngineInstance::Whisper(engine)
```

## Current Verification Procedure

Run these commands from `apps/desktop/src-tauri`:

```bash
cargo check --lib
cargo test --lib polish_engine
cargo test --test polish_engine_test
```

For detailed output:

```bash
cargo test polish_engine -- --nocapture
```

Record the command output in the relevant execution plan or pull request. Do not update this document with estimated coverage or test counts.
