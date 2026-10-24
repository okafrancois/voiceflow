#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { promisify } from 'node:util';

const defaultRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const execFileAsync = promisify(execFile);

export const MLX_BOOTSTRAP = Object.freeze({
  version: '0.13.5',
  url: 'https://github.com/astral-sh/uv/releases/download/0.13.5/uv-aarch64-apple-darwin.tar.gz',
  sha256: '743f85035a1996ecb350659e01cd9faea9709829be09b1b4b03c73eaf3c9cd73',
  binarySha256: '7dad6047467a9d6b081c033cb3efed9ba39a4799c698d09e6cd598f3a6a32338',
  resource: 'bin/apple-silicon/uv',
});

function sha256(contents) {
  return createHash('sha256').update(contents).digest('hex');
}

function verifyChecksum(contents, expectedSha256) {
  const actual = sha256(contents);
  if (actual !== expectedSha256) {
    throw new Error(`MLX bootstrap sha256 mismatch: expected ${expectedSha256}, got ${actual}`);
  }
}

export async function installMlxBootstrapArchive({
  archivePath,
  destinationPath,
  expectedSha256 = MLX_BOOTSTRAP.sha256,
}) {
  verifyChecksum(readFileSync(archivePath), expectedSha256);
  mkdirSync(dirname(destinationPath), { recursive: true });
  const staging = mkdtempSync(join(dirname(destinationPath), '.mlx-bootstrap-'));
  try {
    await execFileAsync('tar', [
      '-xzf', archivePath, '-C', staging, 'uv-aarch64-apple-darwin/uv',
    ], { timeout: 60_000, maxBuffer: 16_384 });
    const extracted = join(staging, 'uv-aarch64-apple-darwin/uv');
    if (!lstatSync(extracted).isFile()) {
      throw new Error('MLX bootstrap archive must contain a regular uv executable');
    }
    const prepared = join(staging, 'uv');
    copyFileSync(extracted, prepared);
    chmodSync(prepared, 0o755);
    renameSync(prepared, destinationPath);
  } catch (error) {
    throw new Error(`Failed to extract MLX bootstrap archive: ${error.message}`, { cause: error });
  } finally {
    rmSync(staging, { recursive: true, force: true });
  }
}

export async function ensureMlxBootstrap({
  platform = process.platform,
  rootDir = defaultRoot,
  assetsDir = resolve(rootDir, '.tmp/mlx-runtime-assets'),
  fetchImpl = globalThis.fetch,
} = {}) {
  if (!['darwin', 'macos'].includes(platform)) {
    return { status: 'skipped', reason: 'apple_silicon_only' };
  }
  const destinationPath = resolve(rootDir, 'apps/desktop/src-tauri', MLX_BOOTSTRAP.resource);
  if (existsSync(destinationPath)
    && lstatSync(destinationPath).isFile()
    && sha256(readFileSync(destinationPath)) === MLX_BOOTSTRAP.binarySha256) {
    chmodSync(destinationPath, 0o755);
    return { status: 'already_present', destinationPath };
  }

  mkdirSync(assetsDir, { recursive: true });
  const archivePath = resolve(assetsDir, `uv-${MLX_BOOTSTRAP.version}-aarch64-apple-darwin.tar.gz`);
  if (existsSync(archivePath) && sha256(readFileSync(archivePath)) !== MLX_BOOTSTRAP.sha256) {
    rmSync(archivePath);
  }
  if (!existsSync(archivePath)) {
    const response = await fetchImpl(MLX_BOOTSTRAP.url, {
      headers: { 'user-agent': 'Voice Flow MLX runtime preparation' },
      redirect: 'follow',
      signal: AbortSignal.timeout(120_000),
    });
    if (!response.ok) {
      throw new Error(`MLX bootstrap download failed: ${response.status} ${response.statusText}`);
    }
    const contents = Buffer.from(await response.arrayBuffer());
    verifyChecksum(contents, MLX_BOOTSTRAP.sha256);
    const staging = mkdtempSync(join(assetsDir, '.download-'));
    try {
      const stagedArchive = join(staging, 'uv.tar.gz');
      writeFileSync(stagedArchive, contents);
      renameSync(stagedArchive, archivePath);
    } finally {
      rmSync(staging, { recursive: true, force: true });
    }
  }
  await installMlxBootstrapArchive({ archivePath, destinationPath });
  verifyChecksum(readFileSync(destinationPath), MLX_BOOTSTRAP.binarySha256);
  return { status: 'prepared', destinationPath };
}

async function main() {
  const args = process.argv.slice(2);
  const platformIndex = args.indexOf('--platform');
  if (platformIndex !== -1 && !args[platformIndex + 1]) {
    throw new Error('--platform requires a value');
  }
  const result = await ensureMlxBootstrap({
    platform: platformIndex === -1 ? process.platform : args[platformIndex + 1],
  });
  console.log(`MLX bootstrap: ${result.status}${result.destinationPath ? ` (${result.destinationPath})` : ''}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
