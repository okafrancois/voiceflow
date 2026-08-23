# Polish Prompt Quality Tests

`polish_prompt_quality_test.rs` contains ignored integration tests for the output quality of local polish prompts.

## Test Cases

### Filler Removal

The filler template should remove hesitation words while preserving meaning and the input language. It also checks that clean text remains unchanged.

Example:

```text
Input:  Um, I think we should, like, go there tomorrow, you know?
Output: I think we should go there tomorrow.
```

### Language Preservation

Each template must return text in the same language as its input. The test covers multiple languages to catch accidental translation.

### Template-Specific Behavior

- The formal template removes casual phrasing.
- The concise template shortens text without dropping key information.
- The agent template produces structured Markdown and preserves requested terms.

## Prerequisite

The tests require the `qwen3.5-0.8b` model at the application's model directory:

```text
~/Library/Application Support/com.ariatype.app/models/qwen3.5-0.8b-q8_0.gguf
```

## Commands

Run from `apps/desktop/src-tauri`:

```bash
# Full quality suite
cargo test --test polish_prompt_quality_test -- --ignored --nocapture

# Filler removal only
cargo test --test polish_prompt_quality_test test_filler_prompt_effectiveness -- --ignored --nocapture

# Language preservation only
cargo test --test polish_prompt_quality_test test_language_preservation_effectiveness -- --ignored --nocapture

# Template-specific behavior only
cargo test --test polish_prompt_quality_test test_template_specific_behavior -- --ignored --nocapture
```

## Why the Tests Are Ignored

- They require a model download of about 1 GB.
- Each inference can take 10–30 seconds.
- Output can vary slightly between model and quantization versions.

## Evaluation Criteria

A result passes when it preserves the input language and meaning, performs the selected edit, and adds no unsupported content. When a test fails, inspect the actual output and the corresponding `system_prompt` in `templates.rs` before changing assertions.
