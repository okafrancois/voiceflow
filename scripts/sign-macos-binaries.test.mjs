import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative } from 'node:path';

function withSigningFixture(run) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'voiceflow macos signing ')));
  const scriptsDir = join(root, 'scripts');
  const commandsDir = join(root, 'commands');
  const callsPath = join(root, 'codesign-calls.jsonl');
  mkdirSync(scriptsDir);
  mkdirSync(commandsDir);
  copyFileSync(
    new URL('./sign-macos-binaries.mjs', import.meta.url),
    join(scriptsDir, 'sign-macos-binaries.mjs'),
  );
  const codesignPath = join(commandsDir, 'codesign');
  writeFileSync(codesignPath, `#!${process.execPath}
const { appendFileSync } = require('node:fs');
appendFileSync(process.env.CODESIGN_CALLS_PATH, JSON.stringify(process.argv.slice(2)) + '\\n');
`);
  chmodSync(codesignPath, 0o755);

  function writeRuntime(name, mode = 0o755) {
    const path = join(root, 'apps/desktop/src-tauri/bin', name);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, 'fixture runtime');
    chmodSync(path, mode);
    return path;
  }

  function sign(identity = 'Fixture Signing Identity') {
    const result = spawnSync(process.execPath, [join(scriptsDir, 'sign-macos-binaries.mjs')], {
      cwd: root,
      encoding: 'utf8',
      env: {
        ...process.env,
        APPLE_SIGNING_IDENTITY: identity,
        CODESIGN_CALLS_PATH: callsPath,
        PATH: commandsDir,
      },
    });
    assert.equal(result.status, 0, result.stderr || result.error?.message);
    const calls = existsSync(callsPath)
      ? readFileSync(callsPath, 'utf8').trim().split('\n').map((line) => JSON.parse(line))
      : [];
    for (const args of calls) {
      assert.deepEqual(args.slice(0, -1), [
        '--force', '--options', 'runtime', '--timestamp', '--entitlements',
        join(root, 'apps/desktop/src-tauri/entitlements.plist'), '--sign', identity,
      ]);
    }
    return calls.map((args) => relative(join(root, 'apps/desktop/src-tauri/bin'), args.at(-1))).sort();
  }

  try {
    run({ root, writeRuntime, sign });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('signs the executable MLX uv bootstrap alongside existing macOS runtime binaries', () => {
  withSigningFixture(({ writeRuntime, sign }) => {
    const expected = [
      'apple-silicon/sense-voice-main-aarch64-apple-darwin',
      'apple-silicon/llama-server',
      'apple-silicon/uv',
      'intel/llama-server',
      'universal/libllama.dylib',
      'macos/libggml.dylib',
    ];
    for (const name of expected) {
      writeRuntime(name, name.endsWith('.dylib') ? 0o644 : 0o755);
    }

    assert.deepEqual(sign(), expected.sort());
  });
});

test('does not sign non-executable uv files or unrelated executables', () => {
  withSigningFixture(({ writeRuntime, sign }) => {
    writeRuntime('apple-silicon/uv', 0o644);
    writeRuntime('apple-silicon/uvx');
    writeRuntime('apple-silicon/uv.backup');
    writeRuntime('intel/unrelated-tool');

    assert.deepEqual(sign(), []);
  });
});

test('does not sign runtime symlinks or directories', () => {
  withSigningFixture(({ root, writeRuntime, sign }) => {
    const executable = writeRuntime('apple-silicon/fixture-tool');
    const runtimeDir = dirname(executable);
    symlinkSync(executable, join(runtimeDir, 'uv'));
    symlinkSync(executable, join(runtimeDir, 'llama-server'));
    symlinkSync(executable, join(runtimeDir, 'linked.dylib'));
    mkdirSync(join(root, 'apps/desktop/src-tauri/bin/intel/uv'), { recursive: true });
    mkdirSync(join(runtimeDir, 'directory.dylib'));

    assert.deepEqual(sign(), []);
  });
});

test('skips signing without an identity even when the uv bootstrap is present', () => {
  withSigningFixture(({ writeRuntime, sign }) => {
    writeRuntime('apple-silicon/uv');

    assert.deepEqual(sign(''), []);
  });
});
