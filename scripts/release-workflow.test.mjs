import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const releaseWorkflow = readFileSync(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8');

test('release workflow pins a llama.cpp runtime release', () => {
  assert.match(releaseWorkflow, /LLAMA_CPP_RELEASE_TAG:\s*b\d+/);
});

test('release workflow prepares the macOS local polish runtimes', () => {
  assert.match(releaseWorkflow, /prepare-llama-server-release-assets\.mjs[\s\S]*--platform macos/);
  assert.match(releaseWorkflow, /\*macos\*arm64\*/);
  assert.match(releaseWorkflow, /\*macos\*x64\*/);
  assert.doesNotMatch(releaseWorkflow, /--platform windows|\*win\*cpu\*x64\*/);
});

test('release workflow requires bundled local polish runtime during packaging', () => {
  const requiredGateCount = releaseWorkflow.match(/ARIATYPE_REQUIRE_LOCAL_POLISH_RUNTIME:\s*"1"/g)
    ?.length ?? 0;

  assert.equal(requiredGateCount, 1);
});

test('release workflow verifies bundled runtime resources before upload', () => {
  assert.match(
    releaseWorkflow,
    /verify-tauri-runtime-resources\.mjs --platform macos --smoke --smoke-timeout-ms 30000/
  );
  assert.doesNotMatch(releaseWorkflow, /verify-tauri-runtime-resources\.mjs --platform windows/);
});

test('release workflow builds and publishes macOS assets only', () => {
  assert.match(releaseWorkflow, /build-macos:[\s\S]*needs: preflight/);
  assert.match(releaseWorkflow, /publish:[\s\S]*needs: build-macos/);
  assert.match(releaseWorkflow, /--require-updater-platform darwin-aarch64/);
  assert.match(releaseWorkflow, /--require-updater-platform darwin-x86_64/);
  assert.doesNotMatch(
    releaseWorkflow,
    /build-windows:|windows-latest|windows-release-assets|windows-x86_64/
  );
});

test('release workflow publishes Voice Flow desktop assets only', () => {
  assert.match(releaseWorkflow, /--title "Voice Flow v\$\{\{ env\.VERSION \}\}"/);
  assert.doesNotMatch(releaseWorkflow, /Homebrew|HOMEBREW_TAP_TOKEN/i);
  assert.doesNotMatch(releaseWorkflow, /Cloudflare|CLOUDFLARE_/i);
  assert.doesNotMatch(releaseWorkflow, /Build website|Deploy website/i);
});

test('release workflow notarizes macOS builds with an App Store Connect API key', () => {
  assert.match(releaseWorkflow, /APPLE_API_ISSUER:\s*\$\{\{ secrets\.APPLE_API_ISSUER \}\}/);
  assert.match(releaseWorkflow, /APPLE_API_KEY:\s*\$\{\{ secrets\.APPLE_API_KEY \}\}/);
  assert.match(releaseWorkflow, /APPLE_API_KEY_P8:\s*\$\{\{ secrets\.APPLE_API_KEY_P8 \}\}/);
  assert.match(releaseWorkflow, /APPLE_API_KEY_PATH=/);
  assert.doesNotMatch(releaseWorkflow, /secrets\.APPLE_ID|secrets\.APPLE_PASSWORD/);
  assert.doesNotMatch(releaseWorkflow, /APPLE_TEAM_ID/);
});

test('release workflow checks required secrets before starting desktop builds', () => {
  assert.match(releaseWorkflow, /preflight:/);
  assert.match(releaseWorkflow, /Missing required release secret/);
  assert.match(releaseWorkflow, /build-macos:[\s\S]*needs: preflight/);
});

test('release workflow uses the repository pnpm version', () => {
  const pnpmVersionCount = releaseWorkflow.match(/version:\s*8\.15\.0/g)?.length ?? 0;

  assert.equal(pnpmVersionCount, 2);
});
