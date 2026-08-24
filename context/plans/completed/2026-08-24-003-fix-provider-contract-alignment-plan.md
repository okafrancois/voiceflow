---
title: Align cloud provider contracts
type: fix
status: completed
date: 2026-08-24
---

# Align cloud provider contracts

## Overview

Make backend dispatch, provider schemas, connection checks, deterministic tests, and provider reference documents describe the same shipped integrations.

## Problem frame

The backend dispatches three cloud STT providers and two cloud Polish providers. The current reference and Cloud Service specification claim additional providers. Live auth tests also run as ordinary tests even though they depend on external network access.

The desired state has one provider list, deterministic local contract coverage for every shipped integration, optional ignored live checks, and no production Volcengine path other than `bigmodel_nostream`.

## Scope boundaries

In scope:

- Cloud STT and Polish provider contracts.
- Provider schema defaults.
- WebSocket and HTTP mock-server tests.
- Optional live contract checks.
- Cloud provider reference and engine contract documentation.

Out of scope:

- New providers.
- Settings UI redesign.
- Credential management.
- Product-wide branding and localization.

## Implementation units

1. Add failing tests for exact schema membership, unsupported Polish providers, and the Volcengine endpoint rule.
2. Remove Volcengine bidirectional mode selection from the production client and reject legacy official endpoints.
3. Add deterministic lifecycle tests for Aliyun and ElevenLabs. Retain the existing deterministic Volcengine lifecycle tests.
4. Add connection-check mock coverage for Anthropic and OpenAI. Mark live credential checks ignored by default.
5. Rewrite the STT and Polish provider references and update stale feature and index entries.
6. Run focused tests, the complete Rust suite, Clippy, and rustfmt.

## System-wide impact

The settings UI continues to consume the backend schema through IPC. Existing saved provider IDs remain unchanged. A saved official Volcengine bidirectional endpoint now fails with a clear migration message instead of using the lower-accuracy interface.

## Risks and dependencies

- Vendor protocol changes cannot be detected by local mocks. Ignored live checks remain available when credentials and network access are present.
- Mock WebSocket servers must assert handshake headers and message order so that a parser-only test cannot hide a broken request.
- Other work may edit product branding in reference docs. Provider-specific edits must be easy to reconcile.

## Verification evidence

- `cargo test --test cloud_stt_provider_contract_test --test cloud_polish_mock_test`: 10 passed, 0 failed.
- `cargo test --test volcengine_streaming_mock_test --test cloud_stt_streaming_lifecycle_test --test cloud_stt_provider_contract_test --test cloud_polish_mock_test --test cloud_provider_api_test`: 22 passed, 0 failed, 10 ignored live checks.
- `cargo fmt -- --check`: passed.
- Vendor live checks were not run. No credentials or vendor access were claimed.
- Full-repository Rust, frontend, and package builds remain part of the parent execution plan because other B/P lots were still editing the shared tree during this slice.
