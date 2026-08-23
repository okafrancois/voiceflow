import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const root = resolve(import.meta.dirname, "..");
const desktopDir = resolve(root, "apps/desktop");

function activeDeveloperDirectory() {
  const result = spawnSync("/usr/bin/xcode-select", ["-p"], {
    encoding: "utf8",
  });
  return result.status === 0 ? result.stdout.trim() : undefined;
}

function hasWorkingXcode(developerDir, baseEnv) {
  if (!developerDir || !existsSync(developerDir)) return false;

  const result = spawnSync("/usr/bin/xcodebuild", ["-version"], {
    env: { ...baseEnv, DEVELOPER_DIR: developerDir },
    stdio: "ignore",
  });
  return result.status === 0;
}

export function resolveDeveloperDir(baseEnv = process.env) {
  const candidates = [
    baseEnv.DEVELOPER_DIR,
    activeDeveloperDirectory(),
    "/Applications/Xcode.app/Contents/Developer",
    "/Applications/Xcode-beta.app/Contents/Developer",
  ];

  for (const candidate of new Set(candidates.filter(Boolean))) {
    if (hasWorkingXcode(candidate, baseEnv)) return candidate;
  }

  throw new Error(
    "A full Xcode installation is required to build Voice Flow Dev on macOS. Install Xcode or set DEVELOPER_DIR to its Contents/Developer directory.",
  );
}

function runRequired(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: desktopDir,
    env: process.env,
    stdio: "inherit",
    ...options,
  });

  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

export function main() {
  runRequired(process.execPath, [
    resolve(root, "scripts/prepare-tauri-runtime-resources.mjs"),
    "--platform",
    "macos",
  ]);

  const buildEnv = {
    ...process.env,
    DEVELOPER_DIR: resolveDeveloperDir(),
  };
  delete buildEnv.APPLE_SIGNING_IDENTITY;
  delete buildEnv.APPLE_TEAM_ID;
  delete buildEnv.APPLE_ID;
  delete buildEnv.APPLE_PASSWORD;

  runRequired(
    resolve(desktopDir, "node_modules/.bin/tauri"),
    [
      "build",
      "--debug",
      "--bundles",
      "app",
      "--config",
      "src-tauri/tauri.dev.conf.json",
      "--config",
      "src-tauri/tauri.macos.unsigned.conf.json",
      "--config",
      "src-tauri/tauri.runtime.generated.conf.json",
    ],
    { env: buildEnv },
  );

  runRequired("/usr/bin/open", [
    resolve(
      desktopDir,
      "src-tauri/target/debug/bundle/macos/Voice Flow Dev.app",
    ),
  ]);
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main();
}
