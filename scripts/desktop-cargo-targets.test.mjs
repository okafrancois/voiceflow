import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';

const cargoManifest = readFileSync(
  new URL('../apps/desktop/src-tauri/Cargo.toml', import.meta.url),
  'utf8',
);
const exportScript = readFileSync(
  new URL('../apps/desktop/scripts/export-mocks.sh', import.meta.url),
  'utf8',
);

test('production bundles exclude the E2E mock exporter binary', () => {
  const autoDiscoveredBinary = new URL(
    '../apps/desktop/src-tauri/src/bin/export-mocks.rs',
    import.meta.url,
  );
  const gatedBinary = new URL(
    '../apps/desktop/src-tauri/dev-bin/export-mocks.rs',
    import.meta.url,
  );

  assert.equal(existsSync(autoDiscoveredBinary), false);
  assert.equal(existsSync(gatedBinary), true);
  assert.match(
    cargoManifest,
    /\[\[bin\]\][\s\S]*name\s*=\s*"export-mocks"[\s\S]*path\s*=\s*"dev-bin\/export-mocks\.rs"[\s\S]*required-features\s*=\s*\["e2e-testing"\]/,
  );
  assert.match(exportScript, /cargo run --features e2e-testing --bin export-mocks/);
});
