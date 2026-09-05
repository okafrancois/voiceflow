# Polish output safety

## Version

0.1.0

## Status

Completed.

## Verification

- Rust unit and integration suite: passed.
- Clippy with all features and warnings denied: passed.
- Rust formatting check: passed.
- Desktop production build, shared TypeScript typecheck, and i18n check: passed.

## Problem

Local instruction models can copy examples from a Polish template instead of transforming the transcript. They can also translate the transcript or remove most of its content. Voice Flow currently accepts these outputs as successful Polish results.

The production regression was reproduced with Gemma 2B IT:

- a 433-character French transcript became a 43-character English example from the Agent template;
- a 242-character French transcript became 105 characters assembled from English examples in the Clean Dictation template.

## Goal

Polish may change presentation, but it must not silently change the transcript language or discard its substance.

## Acceptance criteria

1. Built-in templates contain instructions only, with no input/output examples that a small model can copy.
2. Every Polish request explicitly states the configured source language, or tells the model to detect it from the transcript when the setting is `auto`.
3. French input followed by clearly English output is rejected and the original transcript is used.
4. English input followed by clearly French output is rejected and the original transcript is used.
5. A non-concise Polish result shorter than 55% of a transcript of at least 120 characters is rejected.
6. The Concise template may shorten more aggressively, but output shorter than 30% is rejected.
7. Rejected Polish output is never stored or inserted as the final transcript.
8. Valid punctuation, cleanup, list formatting, and mixed technical vocabulary remain accepted.
9. Local and cloud Polish share the same output-safety decision in the backend.

## Out of scope

- Replacing the selected model automatically.
- General-purpose language identification for every language pair.
- Changing STT model selection.

## Shared policy coverage (2026-09-05)

[Product simplification](../../../product-simplification/0.1.0/prd/erd.md) moves acceptance and provider dispatch into `services/text_transform.rs`. Recording and retries, history re-polish, and quick re-polish use Cleanup or Concise intent. Translation explicitly allows a language change; Reply intentionally allows answering; Rewrite allows more extensive shortening while retaining the applicable language/question guards. Empty output always falls back to the source.

These are bounded regression heuristics, including French/English word scoring and length ratios, not semantic equivalence checks. Original text remains available. Provider-response tests cover history and workflow rejection and explicit translation; existing recording safety tests cover language, shortening, questions, and timing.
