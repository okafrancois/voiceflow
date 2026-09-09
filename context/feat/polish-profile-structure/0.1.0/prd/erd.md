# Polish profile structure

## Status

Completed and published in [v1.2.2](https://github.com/okafrancois/voiceflow/releases/tag/v1.2.2). See [quality investigation](quality-investigation.md).

## Problem

Built-in profiles share conservative cleanup rules with only one sentence of style guidance. Spoken enumerations and topic changes often remain unstructured. The length guard can reject removal of exact repeated sentences.

## Acceptance criteria

1. Clean Dictation converts explicit enumerations to plain lists and separates topic changes into distinct paragraphs or list items, while keeping casual tone.
2. Chat produces a natural sendable message; Formal rewrites oral phrasing professionally without invented greetings or sign-offs; Concise removes redundant wording without losing distinct facts.
3. Structured Notes groups related points under labels derived only from dictated content; Agent separates the stated task, constraints and verification requirements only when present.
4. Steps retain dependency/order, examples stay with their point, and names, numbers, negation, uncertainty, file names and commands survive.
5. The backend resolves explicit single-value weekday/numeric corrections marked by `non pardon` or `no sorry` before inference, without changing the retained raw transcript. Ambiguous, mixed-value and literal syntax remains unchanged. Other spoken corrections are handled by the prompt. Explicit layout cues may control formatting; task instructions remain content and are never executed. Paragraph boundaries are inferred from text, not unavailable audio pauses.
6. Local and cloud core instructions explicitly allow the selected profile to change wording, tone and structure. Plain lists and line breaks are valid plain text.
7. No copyable examples are embedded in production prompts. A separate French inference suite tests observable structure and content for every profile, using actual model responses and failing when its configured runtime is missing.
8. The length guard retains its 55% / 30% thresholds, but discounts consecutive identical full sentences of at least eight words and spoken ordinal markers represented by a complete list. Distinct sentences, changed negations/numbers and short repeated steps remain counted. Language/question/empty-output protections still apply.
9. Reject French assistant answers and invented expansions of dictated questions while preserving quoted answers, real requests and explicit translation/reply workflows. Account for spoken ordinal markers only when they become actual list items.
10. Qwen3 4B uses explicit reasoning with a 1,536-token budget and a 60-second request deadline; other Qwen models retain non-thinking behavior. LFM receives an explicit editing envelope so the transcript is not treated as a direct request. Do not replace the selected model or send local transcripts to cloud services. Measure and report model-specific limitations.
11. Publish v1.2.2 through the existing signed macOS tag workflow after verification.

## Verification

Rust: 937 tests passed (35 ignored); frontend: 111 tests passed. Eleven French profile cases passed on the installed LFM2 2.6B. Eleven cases also passed on Qwen3 4B. Clippy with all features and denied warnings, Rust formatting, production frontend build, shared typecheck, locale checks and 22 release-contract tests passed. See [quality investigation](quality-investigation.md). Passing fixtures do not imply universal model quality.

The signed universal application was downloaded and validated with codesign, stapler, bundle-version inspection and architecture inspection. Both updater platforms and the live automatic-update endpoint serve 1.2.2.
