import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import {
  MLX_BOOTSTRAP,
  ensureMlxBootstrap,
  installMlxBootstrapArchive,
} from './ensure-mlx-runtime.mjs';

function fixture(context) {
  const root = mkdtempSync(join(tmpdir(), 'voiceflow mlx bootstrap '));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const archiveRoot = 'uv-aarch64-apple-darwin';
  const source = join(root, 'source', archiveRoot);
  mkdirSync(source, { recursive: true });
  writeFileSync(join(source, 'uv'), 'fixture uv binary');
  const archivePath = join(root, 'bootstrap.tar.gz');
  execFileSync('tar', ['-czf', archivePath, '-C', dirname(source), archiveRoot]);
  const expectedSha256 = createHash('sha256').update(readFileSync(archivePath)).digest('hex');
  const destinationPath = join(root, 'app resources', 'bin', 'apple-silicon', 'uv');
  return { root, archivePath, expectedSha256, destinationPath };
}

test('bootstrap pins the official Apple ARM uv release and checksum', () => {
  assert.equal(MLX_BOOTSTRAP.version, '0.13.5');
  assert.equal(MLX_BOOTSTRAP.sha256, '743f85035a1996ecb350659e01cd9faea9709829be09b1b4b03c73eaf3c9cd73');
  assert.equal(MLX_BOOTSTRAP.url, 'https://github.com/astral-sh/uv/releases/download/0.13.5/uv-aarch64-apple-darwin.tar.gz');
  assert.equal(MLX_BOOTSTRAP.resource, 'bin/apple-silicon/uv');
});

test('MLX dependency lock pins every transitive package with SHA256 hashes', () => {
  const input = readFileSync(new URL('../apps/desktop/src-tauri/src/stt_engine/mlx/requirements.in', import.meta.url), 'utf8');
  assert.match(input, /^mlx-audio\[stt\]==0\.6\.1$/m);
  const lock = readFileSync(new URL('../apps/desktop/src-tauri/src/stt_engine/mlx/requirements.lock', import.meta.url), 'utf8');
  const records = lock.split(/\n(?=[a-zA-Z0-9])/).filter((record) => /^[a-zA-Z0-9]/.test(record));
  assert.ok(records.length >= 30);
  for (const record of records) {
    assert.match(record, /^[a-zA-Z0-9][a-zA-Z0-9_.-]*==[^\s]+ \\\n/);
    assert.match(record, /--hash=sha256:[0-9a-f]{64}/);
  }
  for (const dependency of ['mlx-audio==0.6.1', 'mlx==0.32.4', 'mlx-metal==0.32.4', 'numpy==', 'transformers==', 'zstandard==']) {
    assert.ok(lock.includes(dependency), dependency);
  }
});

test('Windows and Linux never download or prepare an ARM bootstrap', async (context) => {
  const { root } = fixture(context);
  for (const platform of ['win32', 'windows', 'linux']) {
    const result = await ensureMlxBootstrap({
      platform,
      rootDir: root,
      fetchImpl: () => assert.fail('non-macOS must not download uv'),
    });
    assert.equal(result.status, 'skipped');
  }
  assert.equal(existsSync(join(root, 'apps')), false);
});

test('verified archive extraction supports spaces without executing the ARM binary', async (context) => {
  const options = fixture(context);
  await installMlxBootstrapArchive(options);
  assert.equal(readFileSync(options.destinationPath, 'utf8'), 'fixture uv binary');
  assert.notEqual(statSync(options.destinationPath).mode & 0o111, 0);
});

test('archive checksum failure does not overwrite an existing bootstrap', async (context) => {
  const options = fixture(context);
  mkdirSync(dirname(options.destinationPath), { recursive: true });
  writeFileSync(options.destinationPath, 'previous binary');
  await assert.rejects(
    installMlxBootstrapArchive({ ...options, expectedSha256: '0'.repeat(64) }),
    /sha256 mismatch/i,
  );
  assert.equal(readFileSync(options.destinationPath, 'utf8'), 'previous binary');
});

test('corrupt macOS downloads are rejected before packaging', async (context) => {
  const { root } = fixture(context);
  let requested;
  await assert.rejects(ensureMlxBootstrap({
    platform: 'macos',
    rootDir: root,
    fetchImpl: async (url) => {
      requested = url;
      return new Response('not the official archive');
    },
  }), /sha256 mismatch/i);
  assert.equal(requested, MLX_BOOTSTRAP.url);
  assert.equal(existsSync(join(root, 'apps/desktop/src-tauri', MLX_BOOTSTRAP.resource)), false);
});

test('bootstrap download errors preserve HTTP context', async (context) => {
  const { root } = fixture(context);
  await assert.rejects(ensureMlxBootstrap({
    platform: 'darwin',
    rootDir: root,
    fetchImpl: async () => new Response('', { status: 503, statusText: 'Unavailable' }),
  }), /503.*Unavailable/);
});

test('unsafe archive members cannot replace the pinned executable', async (context) => {
  const options = fixture(context);
  execFileSync('tar', ['-czf', options.archivePath, '-C', options.root, 'source']);
  options.expectedSha256 = createHash('sha256').update(readFileSync(options.archivePath)).digest('hex');
  await assert.rejects(installMlxBootstrapArchive(options), /extract|archive/i);
  assert.equal(existsSync(options.destinationPath), false);
});
