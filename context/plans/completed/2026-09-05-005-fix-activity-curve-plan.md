---
title: Restore the smooth daily activity curve
type: fix
status: completed
date: 2026-09-05
---

## Overview and problem frame

The user prefers the previous smooth activity curve to the replacement bars.
Restore that visual treatment over the current backend statistics.

## Scope boundaries and system-wide impact

Presentation only: smooth curve, subtle fill, exact point labels and existing
periods/table. No backend, dependency, release tag or metric changes.

## Implementation units

- [x] Identify a failing accessible chart check.
- [x] Replace the bars with a bounded smooth SVG curve.
- [x] Verify chart tests, TypeScript, build and visual output.

## Risks and dependencies

Do not invent negative values or a multi-day trend for a single point.
All-history points still represent sparse active dates.

## Verification evidence

- Regression test failed first because the activity chart had no image role.
- Statistics page tests: 5 passed, including accessible daily values.
- TypeScript and production frontend build passed.
- All locale key checks passed.
- Visually inspected an SVG rendered from the actual component using seven
  synthetic daily values: smooth bounded curve, subtle fill and light grid.
- Native application was not rebuilt or relaunched for this presentation change.

## Workflow extraction

Keep chart interpolation in the presentation layer. Reuse backend daily values
and the existing accessible table; restoring a curve requires no chart dependency.
