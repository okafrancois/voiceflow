#!/usr/bin/env node

import { existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const defaultRoot = resolve(__dirname, '..');
const UNIVERSAL_MACOS_TARGET = 'universal-apple-darwin';

export function universalCargoBinaryPaths({
  rootDir = defaultRoot,
  binaryName = 'voiceflow-cli',
  profile = 'release',
} = {}) {
  const targetDir = resolve(rootDir, 'apps/desktop/src-tauri/target');
  return {
    inputs: [
      resolve(targetDir, 'aarch64-apple-darwin', profile, binaryName),
      resolve(targetDir, 'x86_64-apple-darwin', profile, binaryName),
    ],
    output: resolve(targetDir, UNIVERSAL_MACOS_TARGET, profile, binaryName),
  };
}

export function prepareUniversalCargoBinary({
  rootDir = defaultRoot,
  binaryName = 'voiceflow-cli',
  profile = 'release',
  targetTriple = process.env.TAURI_ENV_TARGET_TRIPLE ?? '',
  spawn = spawnSync,
} = {}) {
  if (targetTriple !== UNIVERSAL_MACOS_TARGET) {
    return { status: 'skipped', reason: 'not_universal_macos' };
  }

  const paths = universalCargoBinaryPaths({ rootDir, binaryName, profile });
  const missingInput = paths.inputs.find((input) => !existsSync(input));
  if (missingInput) {
    throw new Error(`Missing architecture-specific binary: ${missingInput}`);
  }

  const result = spawn(
    '/usr/bin/lipo',
    ['-create', ...paths.inputs, '-output', paths.output],
    { stdio: 'inherit' }
  );
  if (result.error) {
    throw new Error(`Failed to run lipo: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`lipo failed with exit code ${result.status ?? 'unknown'}`);
  }

  return { status: 'prepared', output: paths.output };
}

function main() {
  const profile = process.env.TAURI_ENV_DEBUG === 'true' ? 'debug' : 'release';
  const result = prepareUniversalCargoBinary({ profile });
  if (result.status === 'prepared') {
    console.log(`Prepared universal bundled CLI: ${result.output}`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
