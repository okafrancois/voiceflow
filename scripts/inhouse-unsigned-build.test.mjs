import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');

test('desktop unsigned mac build script merges in-house and unsigned configs', () => {
  const packageJson = JSON.parse(
    readFileSync(resolve(root, 'apps/desktop/package.json'), 'utf8')
  );

  assert.equal(
    packageJson.scripts['tauri:build:mac:unsigned'],
    'node ../../scripts/prepare-tauri-runtime-resources.mjs --platform macos --require-runtime && env -u APPLE_SIGNING_IDENTITY -u APPLE_TEAM_ID -u APPLE_ID -u APPLE_PASSWORD tauri build --config src-tauri/tauri.dev.conf.json --config src-tauri/tauri.macos.unsigned.conf.json --config src-tauri/tauri.runtime.generated.conf.json'
  );
});

test('macOS permission testing launches an entitled app bundle', () => {
  const packageJson = JSON.parse(
    readFileSync(resolve(root, 'apps/desktop/package.json'), 'utf8')
  );
  const command = packageJson.scripts['tauri:dev:mac-permissions'];
  const script = readFileSync(
    resolve(root, 'scripts/run-macos-permission-dev.mjs'),
    'utf8'
  );

  assert.equal(command, 'node ../../scripts/run-macos-permission-dev.mjs');
  assert.match(script, /prepare-tauri-runtime-resources\.mjs/);
  assert.match(script, /["']--platform["'][\s\S]*["']macos["']/);
  assert.doesNotMatch(script, /--require-runtime/);
  assert.match(script, /["']build["'][\s\S]*["']--debug["']/);
  assert.match(script, /["']--bundles["'][\s\S]*["']app["']/);
  assert.match(script, /tauri\.macos\.unsigned\.conf\.json/);
  assert.match(script, /Xcode-beta\.app/);
  assert.match(script, /["']\/usr\/bin\/open["']/);
  assert.match(script, /Voice Flow Dev\.app/);
});

test('multi-platform unsigned mac commands merge in-house and unsigned configs', () => {
  const script = readFileSync(resolve(root, 'scripts/build-all-platforms.mjs'), 'utf8');

  assert.match(
    script,
    /tauri\.dev\.conf\.json --config src-tauri\/tauri\.macos\.unsigned\.conf\.json --config \$\{runtimeConfig\} --target aarch64-apple-darwin/
  );
  assert.match(
    script,
    /tauri\.dev\.conf\.json --config src-tauri\/tauri\.macos\.unsigned\.conf\.json --config \$\{runtimeConfig\} --target x86_64-apple-darwin/
  );
});
