import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import test from 'node:test';

import {
  prepareUniversalCargoBinary,
  universalCargoBinaryPaths,
} from './prepare-tauri-bundle-binaries.mjs';

const tauriConfig = JSON.parse(
  readFileSync(new URL('../apps/desktop/src-tauri/tauri.conf.json', import.meta.url), 'utf8')
);

test('Tauri prepares additional Cargo binaries immediately before bundling', () => {
  assert.equal(
    tauriConfig.build.beforeBundleCommand,
    'node ../../scripts/prepare-tauri-bundle-binaries.mjs'
  );
});

test('universalCargoBinaryPaths resolves both architecture inputs and the universal output', () => {
  const rootDir = '/workspace/voiceflow';

  assert.deepEqual(universalCargoBinaryPaths({ rootDir, binaryName: 'voiceflow-cli' }), {
    inputs: [
      '/workspace/voiceflow/apps/desktop/src-tauri/target/aarch64-apple-darwin/release/voiceflow-cli',
      '/workspace/voiceflow/apps/desktop/src-tauri/target/x86_64-apple-darwin/release/voiceflow-cli',
    ],
    output:
      '/workspace/voiceflow/apps/desktop/src-tauri/target/universal-apple-darwin/release/voiceflow-cli',
  });
});

test('prepareUniversalCargoBinary merges the architecture-specific CLI binaries with lipo', () => {
  const rootDir = mkdtempSync(resolve(tmpdir(), 'voiceflow-universal-cli-'));
  const paths = universalCargoBinaryPaths({ rootDir });
  for (const input of paths.inputs) {
    mkdirSync(dirname(input), { recursive: true });
    writeFileSync(input, 'binary');
  }

  const calls = [];
  const result = prepareUniversalCargoBinary({
    rootDir,
    targetTriple: 'universal-apple-darwin',
    spawn(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0 };
    },
  });

  assert.equal(result.status, 'prepared');
  assert.deepEqual(calls, [
    {
      command: '/usr/bin/lipo',
      args: ['-create', ...paths.inputs, '-output', paths.output],
      options: { stdio: 'inherit' },
    },
  ]);
});

test('prepareUniversalCargoBinary skips non-universal builds', () => {
  const result = prepareUniversalCargoBinary({
    rootDir: '/workspace/voiceflow',
    targetTriple: 'aarch64-apple-darwin',
    spawn() {
      throw new Error('lipo must not run');
    },
  });

  assert.deepEqual(result, {
    status: 'skipped',
    reason: 'not_universal_macos',
  });
});

test('prepareUniversalCargoBinary rejects a missing architecture-specific binary', () => {
  const rootDir = mkdtempSync(resolve(tmpdir(), 'voiceflow-universal-cli-missing-'));
  const paths = universalCargoBinaryPaths({ rootDir });
  mkdirSync(dirname(paths.inputs[0]), { recursive: true });
  writeFileSync(paths.inputs[0], 'binary');

  assert.throws(
    () =>
      prepareUniversalCargoBinary({
        rootDir,
        targetTriple: 'universal-apple-darwin',
      }),
    /Missing architecture-specific binary.*x86_64-apple-darwin/s
  );
});
