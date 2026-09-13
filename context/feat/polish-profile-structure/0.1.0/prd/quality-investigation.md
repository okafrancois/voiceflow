# Polish profile quality investigation

## v1.2.2 outcome (superseded)

Version 1.2.2 gave all six profiles explicit rewriting and layout instructions. Local model-family defaults share the canonical example-free Clean Dictation prompt. The backend resolves clear weekday/numeric corrections before inference and retains the raw transcript for history and fallback.

The installed LFM2-2.6B-Q4_K_M passed all eleven French profile cases after the editing envelope and correction preparation were added. Qwen3-4B-Q4_K_M also passed all eleven cases with bounded reasoning. The LFM suite took 11.56 seconds and the Qwen suite 255.66 seconds on this machine while build verification was also running; these are whole-suite durations, not latency guarantees. This is sample-based quality evidence, not a general semantic guarantee.

## Baseline and root causes

The original Clean Dictation, Structured Notes and Agent prompts all returned this enumeration unchanged on Qwen:

> Je veux vérifier trois points. Premièrement, le tracking des événements. Deuxièmement, la sécurité des données. Troisièmement, les nouvelles fonctionnalités de la version 1.2.2.

Longer prompts alone did not fix the problem. The first investigation stopped after three failed evaluation rounds and reverted its unverified prompts. The user then approved model-specific instructions and stronger output validation.

- Qwen3 4B followed list and correction instructions when reasoning was enabled. Unrestricted reasoning could exceed the timeout. The final request explicitly enables reasoning with `thinking_budget_tokens: 1536` and uses a 60-second deadline. The request field is verified against the shipped llama.cpp b9568 source.
- LFM sometimes answered the dictated question. Framing the user message as quoted text to edit corrected that behavior in the evaluated cases.
- LFM still mishandled explicit weekday corrections. The shared backend now resolves only unambiguous weekday/numeric corrections before inference. Multiword, mixed-value, sentence-separated and literal syntax remains untouched.
- The 55% length guard could reject valid lists after ordinal markers disappeared, or valid deletion of repeated sentences. It now discounts those bounded cases without changing the thresholds.

## Reproducible inference check

The suite lives in `apps/desktop/src-tauri/src/polish_engine/profile_quality_tests.rs`. It applies the same transcript preparation, real local HTTP request construction and shared acceptance policy used by the product. No cloud service or user recording was used. Tests run against the installed Voice Flow llama.cpp runtime on isolated loopback ports.

From `apps/desktop/src-tauri`, with a real runtime already serving the selected model:

```sh
VOICEFLOW_QUALITY_BASE_URL=http://127.0.0.1:18082/v1 VOICEFLOW_QUALITY_MODEL=qwen3-4b cargo test --lib french_profile_quality -- --ignored --nocapture
VOICEFLOW_QUALITY_BASE_URL=http://127.0.0.1:18083/v1 VOICEFLOW_QUALITY_MODEL=lfm2-2.6b cargo test --lib french_profile_quality -- --ignored --nocapture
```

`VOICEFLOW_QUALITY_CASE` optionally selects one named case for diagnosis; unmatched names fail. Missing runtime/model configuration fails when the ignored suite is explicitly invoked.

The eleven evaluations cover all six profiles: explicit lists, topic changes, self-correction, professional requests, concision, commands and uncertainty, and dictated questions. The original assertions accepted equivalent French wording (`tracking`/`suivi`, `pas confirmé`/`non confirmé`) and either paragraphs or distinct list items for topic separation. Accepting list items for topic separation was a validation error: it concealed unwanted lists. Formal writing is not required to be shorter; that requirement applies to Concise. Equivalent wording remains acceptable, but formatting checks must distinguish prose from lists. Every model result is printed for human inspection.

## Deterministic verification and limits

- Rust suite: 937 passed, 35 ignored. External-model checks are separately invoked.
- Frontend: 111 passed. Release contracts: 22 passed.
- Failing-first regressions cover duplicate-sentence cleanup, valid list acceptance, French assistant replies, invented questions, lost explicit corrections and pre-inference correction resolution.
- Local HTTP tests cover Qwen thinking fields, the LFM transcript envelope and exclusion of reasoning deltas from visible previews.
- Family-default tests now require the canonical example-free prompt instead of requiring the old copyable continuation example, which contradicted the output-safety specification.

Length ratios, question counts and lexical correction guards are bounded heuristics. They cannot prove that arbitrary model output preserves every fact. External servers may ignore the llama.cpp reasoning-budget extension; the deadline still applies. Other model families and cloud models were not evaluated with real inference in this task. No selected model is replaced automatically.

## v1.2.3 regression and authorized rollback

The user reported unwanted lists in ordinary speech after v1.2.2. Shared list imperatives and repeated profile-specific bullet instructions overrode the weak prose guidance. The original suite lacked negative list assertions and even accepted a bullet before a single question.

New real-inference checks reproduced the problem in all six profiles for an ordinary narrative. Three prompt-only candidates could not jointly preserve prose, explicit enumerations and paragraph breaks on LFM. Their changes were reverted under the recovery protocol. The user then explicitly approved restoring the v1.2.1 prompts, accepting their weaker automatic formatting.

The six built-in definitions and shared local/cloud core prompts now exactly match v1.2.1. The 1.2.2 correction preparation, acceptance guards, LFM envelope and Qwen reasoning policy remain. Structured Notes retains its historical list-oriented instruction, including a possible inline bullet in an ordinary narrative; this rollback is not a claim that every profile always avoids lists.

The full `french_profile_quality` suite retains the stronger target, adds negative cases for all six profiles, and requires actual paragraphs for topic separation. The separate `french_prose_regression` release gate tests delivered text for three ordinary-input cases across the five non-notes profiles. It uses the same acceptance/fallback policy as production; final runs used no fallback. Fifteen cases passed on LFM2 2.6B in 19.38 seconds and fifteen on Qwen3 4B in 218.11 seconds. These durations are observations, not latency guarantees.

```sh
VOICEFLOW_QUALITY_BASE_URL=http://127.0.0.1:18082/v1 VOICEFLOW_QUALITY_MODEL=qwen3-4b cargo test --lib french_prose_regression -- --ignored --nocapture
VOICEFLOW_QUALITY_BASE_URL=http://127.0.0.1:18083/v1 VOICEFLOW_QUALITY_MODEL=lfm2-2.6b cargo test --lib french_prose_regression -- --ignored --nocapture
```

Future formatting work must require both positive and negative cases before claiming improvement. A passing fallback is evidence about delivered-text safety, not about model editing quality. The full stricter suite is a deferred target for this user-authorized rollback.
