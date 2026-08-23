import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const desktopRoot = resolve(root, 'apps/desktop');
const tauriRoot = resolve(desktopRoot, 'src-tauri');

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
    assert.doesNotMatch(source, /AriaType/, localeFile);
  }

  for (const relativePath of ['index.html', 'pill.html', 'src/components/Home/About.tsx']) {
    const source = readFileSync(resolve(desktopRoot, relativePath), 'utf8');
    assert.doesNotMatch(source, /AriaType/, relativePath);
  }
});

test('the canonical logo source identifies the Voice Flow V mark', () => {
  const logoPath = resolve(desktopRoot, 'assets/voice-flow-logo.svg');

  assert.ok(existsSync(logoPath), 'voice-flow-logo.svg should exist');
  const logo = readFileSync(logoPath, 'utf8');
  assert.match(logo, /id="voice-flow-v"/);
  assert.match(logo, /aria-label="Voice Flow V logo"/);
});
