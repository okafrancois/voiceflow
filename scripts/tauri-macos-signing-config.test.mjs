import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const baseConfig = JSON.parse(
  readFileSync(new URL('../apps/desktop/src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
);

test('signed macOS builds receive signing and notarization credentials from the environment', () => {
  const macOS = baseConfig.bundle?.macOS;

  assert.equal(macOS?.signingIdentity, undefined);
  assert.equal(macOS?.providerShortName, undefined);
});
