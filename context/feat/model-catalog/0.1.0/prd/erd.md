# Visual model selectors — 0.1.0

Status: Completed, 2026-10-24. Evidence: [execution plan](../../../../plans/completed/2026-10-24-003-feat-model-catalog-plan.md).

## Problem and goal

The existing selectors hide comparison data in text and repeat large bordered cards. Implement the user-approved interactive proposal as compact, aligned model lists for transcription and polish.

## First-principles model

Users need to see the configured engine, compare catalogue guidance, narrow alternatives by disk size and language or use, and select or manage a model without confusing downloaded files with runtime readiness.

## Information architecture

- Preserve Voice Input, Polish and Performance navigation.
- Keep the configured model above the filters; an enabled speech service, including MLX Audio, takes precedence over the native fallback model.
- Place size, language/use, availability and sort controls above flat rows.
- Show transcription speed and accuracy as aligned segmented bars. Show polish latency classes and backend-recommended templates instead of an invented quality score.
- Keep download progress, cancellation, deletion, compatibility warnings and runtime diagnostics functional. Put secondary runtime details below the catalogue.

## Data contract

- Native `ModelInfo` adds `language_codes: string[]` from its canonical backend definition; empty means unrestricted in the existing catalogue. The field is additive for older fixtures.
- Polish size filtering formats/parses the existing size display metadata; missing sizes are unknown rather than zero. Built-in entries have no download size.
- Latency class, template recommendations, compatibility, availability and download events remain backend-owned.
- Filters, sorting and expanded-list state are transient presentation state only. Model selection goes through existing typed settings commands; switching to a native STT model disables the speech service after saving the native selection.
- Catalogue ratings are labelled indicative, with no hardware benchmark or fabricated MLX scores. Runtime availability is never inferred from file size.

## Acceptance criteria

1. Size and ready-only filters combine and never change the configured model.
2. Language filtering excludes a language-restricted native model when appropriate, using backend metadata.
3. Polish use filters use the backend's recommended template IDs.
4. The selected model remains visible above the filtered list.
5. Download/cancel/delete and built-in protections still work, including failure feedback.
6. A downloaded alternative can be selected directly. Active and downloaded are distinct states.
7. The layout works at narrow modal widths, in both themes, with accessible labels and text alternatives for gauges.
8. All new UI text is translated in the ten shipped locales.

## BDD scenarios

- Given small and large models, when a user chooses under 500 MB and ready-only, then only ready small models appear and selection is unchanged.
- Given French dictation and SenseVoice's restricted languages, when current-language filtering is selected, then SenseVoice is hidden; all languages restores it.
- Given backend polish recommendations, when Documents is chosen, then only models recommended for `document` appear.
- Given MLX Audio enabled, when native models are filtered, then the MLX configuration remains the active summary and no native fallback is labelled active.

## Verification

Failing-first component checks for filters and selection, catalogue metadata checks, frontend tests/build/i18n, Rust checks for touched backend metadata, visual browser/native inspection, and a signed local Dev installation.
