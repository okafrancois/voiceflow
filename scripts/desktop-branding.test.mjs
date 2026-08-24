import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { relative, resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const desktopRoot = resolve(root, 'apps/desktop');
const tauriRoot = resolve(desktopRoot, 'src-tauri');
const retiredProductToken = ['aria', 'type'].join('');
const ignoredDirectoryNames = new Set([
  '.git',
  '.logs',
  '.next',
  'dist',
  'node_modules',
  'out',
  'target',
]);
const textFileExtensions = new Set([
  '',
  '.css',
  '.d.ts',
  '.html',
  '.icns.json',
  '.js',
  '.json',
  '.lock',
  '.md',
  '.mjs',
  '.plist',
  '.rs',
  '.sh',
  '.svg',
  '.toml',
  '.ts',
  '.tsx',
  '.txt',
  '.yaml',
  '.yml',
]);

function walkFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isDirectory() && ignoredDirectoryNames.has(entry.name)) {
      continue;
    }

    const entryPath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(entryPath));
    } else if (entry.isFile()) {
      files.push(entryPath);
    }
  }
  return files;
}

function isTextSource(path) {
  const name = path.split('/').at(-1) ?? '';
  const extension = name.includes('.') ? `.${name.split('.').slice(1).join('.')}` : '';
  if (textFileExtensions.has(extension)) {
    return true;
  }
  return [...textFileExtensions].some((candidate) => candidate && name.endsWith(candidate));
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

test('desktop bundle uses the Voice Flow product identity', () => {
  const production = readJson(resolve(tauriRoot, 'tauri.conf.json'));
  const development = readJson(resolve(tauriRoot, 'tauri.dev.conf.json'));
  const e2e = readJson(resolve(tauriRoot, 'tauri.e2e.conf.json'));

  assert.equal(production.productName, 'Voice Flow');
  assert.equal(production.identifier, 'com.voiceflow.voicetotext');
  assert.equal(development.productName, 'Voice Flow Dev');
  assert.equal(development.identifier, 'com.voiceflow.voicetotext.dev');
  assert.equal(e2e.productName, 'Voice Flow E2E');
  assert.equal(e2e.identifier, 'com.voiceflow.voicetotext.e2e');
});

test('desktop user-facing copy no longer exposes the old product name', () => {
  const localeDir = resolve(desktopRoot, 'src/i18n/locales');
  const localeFiles = readdirSync(localeDir).filter((file) => file.endsWith('.json'));

  for (const localeFile of localeFiles) {
    const localePath = resolve(localeDir, localeFile);
    const source = readFileSync(localePath, 'utf8');
    const translations = JSON.parse(source);

    assert.equal(translations['app.name'], 'Voice Flow', localeFile);
    assert.ok(!source.toLowerCase().includes(retiredProductToken), localeFile);
  }

  for (const relativePath of ['index.html', 'pill.html', 'src/components/Home/About.tsx']) {
    const source = readFileSync(resolve(desktopRoot, relativePath), 'utf8');
    assert.ok(!source.toLowerCase().includes(retiredProductToken), relativePath);
  }
});

test('repository sources and file names use only the Voice Flow identity', () => {
  const violations = [];

  for (const path of walkFiles(root)) {
    const repositoryPath = relative(root, path);
    if (repositoryPath.toLowerCase().includes(retiredProductToken)) {
      violations.push(`${repositoryPath}: retired token in file name`);
    }

    if (!isTextSource(path) || statSync(path).size > 5_000_000) {
      continue;
    }

    const source = readFileSync(path, 'utf8');
    if (source.toLowerCase().includes(retiredProductToken)) {
      violations.push(`${repositoryPath}: retired token in source`);
    }
  }

  assert.deepEqual(violations, []);
});

test('workspace and Rust package identifiers use the voiceflow namespace', () => {
  const rootPackage = readJson(resolve(root, 'package.json'));
  const desktopPackage = readJson(resolve(desktopRoot, 'package.json'));
  const sharedPackage = readJson(resolve(root, 'packages/shared/package.json'));
  const harnessPackage = readJson(resolve(root, 'packages/e2e-harness/package.json'));
  const websitePackage = readJson(resolve(root, 'packages/website/package.json'));
  const cargoManifest = readFileSync(resolve(tauriRoot, 'Cargo.toml'), 'utf8');

  assert.equal(rootPackage.name, 'voiceflow-monorepo');
  assert.equal(desktopPackage.name, '@voiceflow/desktop');
  assert.equal(sharedPackage.name, '@voiceflow/shared');
  assert.equal(harnessPackage.name, '@voiceflow/e2e-harness');
  assert.equal(websitePackage.name, '@voiceflow/website');
  assert.match(cargoManifest, /^name = "voiceflow"$/m);
  assert.match(cargoManifest, /^default-run = "voiceflow"$/m);
  assert.match(cargoManifest, /^name = "voiceflow_lib"$/m);
});

test('language-neutral UI and scripts do not hard-code Han characters', () => {
  const roots = [
    resolve(desktopRoot, 'src'),
    resolve(root, 'packages/website/src'),
    resolve(root, 'scripts'),
  ];
  const allowedLocalePaths = new Set([
    resolve(desktopRoot, 'src/i18n/locales/zh.json'),
    resolve(desktopRoot, 'src/i18n/locales/ja.json'),
    resolve(root, 'packages/website/src/i18n/locales/zh.json'),
    resolve(root, 'packages/website/src/app/[lang]/layout.tsx'),
  ]);
  const violations = [];

  for (const sourceRoot of roots) {
    for (const path of walkFiles(sourceRoot)) {
      if (!isTextSource(path) || allowedLocalePaths.has(path)) {
        continue;
      }
      const source = readFileSync(path, 'utf8');
      if (/\p{Script=Han}/u.test(source)) {
        violations.push(relative(root, path));
      }
    }
  }

  assert.deepEqual(violations, []);
});

test('language selectors use Latin labels outside translated locale files', () => {
  const selectorSources = [
    resolve(desktopRoot, 'src/i18n/index.ts'),
    resolve(desktopRoot, 'src/lib/lang-codes.json'),
  ];

  for (const path of selectorSources) {
    const source = readFileSync(path, 'utf8');
    assert.doesNotMatch(source, /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}]/u);
  }
});

test('backend tooltip sources do not expose Han characters', () => {
  const tooltipSources = [
    resolve(tauriRoot, 'src/commands/audio/start.rs'),
    resolve(tauriRoot, 'src/correction_learning/observer.rs'),
    resolve(tauriRoot, 'src/events/mod.rs'),
  ];

  for (const path of tooltipSources) {
    const productionSource = readFileSync(path, 'utf8').split('#[cfg(test)]', 1)[0];
    assert.doesNotMatch(productionSource, /\p{Script=Han}/u);
  }
});

test('the canonical logo source identifies the Voice Flow V mark', () => {
  const logoPath = resolve(desktopRoot, 'assets/voice-flow-logo.svg');

  assert.ok(existsSync(logoPath), 'voice-flow-logo.svg should exist');
  const logo = readFileSync(logoPath, 'utf8');
  assert.match(logo, /id="voice-flow-v"/);
  assert.match(logo, /aria-label="Voice Flow V logo"/);
});
