import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';

const workflow = readFileSync(new URL('../.github/workflows/test.yml', import.meta.url), 'utf8');
const websiteWorkflowUrl = new URL('../.github/workflows/deploy-website.yml', import.meta.url);

test('desktop CI runs on master and can be started manually', () => {
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /push:[\s\S]*branches:\s*\[master\]/);
  assert.match(workflow, /pull_request:[\s\S]*branches:\s*\[master\]/);
});

test('desktop CI watches backend, frontend, and shared desktop dependencies', () => {
  assert.match(workflow, /\.github\/workflows\/release\.yml/);
  assert.match(workflow, /apps\/desktop\/\*\*/);
  assert.match(workflow, /packages\/shared\/\*\*/);
  assert.match(workflow, /pnpm-lock\.yaml/);
});

test('desktop CI validates frontend contracts', () => {
  assert.match(workflow, /pnpm --filter @ariatype\/desktop build/);
  assert.match(workflow, /pnpm --filter @ariatype\/shared typecheck/);
  assert.match(workflow, /pnpm check:i18n/);
  assert.match(workflow, /version:\s*8\.15\.0/);
});

test('desktop CI keeps Rust checks blocking and serializes tests', () => {
  assert.doesNotMatch(workflow, /\|\| true/);
  assert.match(workflow, /--test-threads=1/);
});

test('website deployment workflow is disabled', () => {
  assert.equal(existsSync(websiteWorkflowUrl), false);
});
