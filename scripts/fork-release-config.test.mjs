import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const releaseFiles = [
  '../.github/workflows/release.yml',
  '../apps/desktop/src-tauri/tauri.updater.conf.json',
  './build-all-platforms.mjs',
  './generate-release-manifests.mjs',
];

test('production release configuration points to the Voice Flow fork', () => {
  for (const file of releaseFiles) {
    const contents = readFileSync(new URL(file, import.meta.url), 'utf8');

    assert.doesNotMatch(contents, /github\.com\/joe223\/AriaType/);
  }

  const updaterConfig = readFileSync(
    new URL('../apps/desktop/src-tauri/tauri.updater.conf.json', import.meta.url),
    'utf8',
  );
  assert.match(updaterConfig, /github\.com\/okafrancois\/voiceflow/);
});
