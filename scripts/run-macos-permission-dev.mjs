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

function readPlist(path, input) {
  const result = spawnSync(
    "/usr/bin/plutil",
    ["-convert", "json", "-o", "-", path],
    { encoding: "utf8", input },
  );
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || `Unable to read plist: ${path}`);
  }
  return JSON.parse(result.stdout);
}

export function parseArguments(args) {
  let open = true;

  for (const argument of args) {
    if (argument === "--no-open") {
      open = false;
      continue;
    }
    throw new Error(`Unknown argument: ${argument}`);
  }

  return { open };
}

export function localInstallBuildArguments() {
  return [
    "build",
    "--debug",
    "--bundles",
    "app",
    "--config",
    "src-tauri/tauri.dev.conf.json",
    "--config",
    "src-tauri/tauri.macos.unsigned.conf.json",
    "--config",
    "src-tauri/tauri.local-install.conf.json",
    "--config",
    "src-tauri/tauri.runtime.generated.conf.json",
  ];
}

export function assertLocalInstallMetadata({
  identifier,
  urlSchemes,
  audioInputEntitlement,
}) {
  if (identifier !== "com.voiceflow.voicetotext.dev") {
    throw new Error(`Unexpected development bundle identifier: ${identifier}`);
  }
  if (
    urlSchemes.length !== 1 ||
    urlSchemes[0] !== "voiceflow-dev"
  ) {
    throw new Error(
      `Unexpected development URL schemes: ${urlSchemes.join(", ")}`,
    );
  }
  if (audioInputEntitlement !== true) {
    throw new Error("The audio-input entitlement must be true.");
  }
}

export function verifyBundle(appPath) {
  runRequired("/usr/bin/codesign", [
    "--verify",
    "--deep",
    "--strict",
    "--verbose=2",
    appPath,
  ]);

  const info = readPlist(resolve(appPath, "Contents/Info.plist"));
  const urlSchemes = (info.CFBundleURLTypes ?? []).flatMap(
    (entry) => entry.CFBundleURLSchemes ?? [],
  );

  const entitlements = spawnSync(
    "/usr/bin/codesign",
    ["-d", "--entitlements", ":-", appPath],
    { encoding: "utf8" },
  );
  if (entitlements.status !== 0) {
    process.stderr.write(entitlements.stderr ?? "");
    process.exit(entitlements.status ?? 1);
  }
  const entitlementPlist = readPlist("-", entitlements.stdout);

  assertLocalInstallMetadata({
    identifier: info.CFBundleIdentifier,
    urlSchemes,
    audioInputEntitlement:
      entitlementPlist["com.apple.security.device.audio-input"],
  });
}

export function main(args = process.argv.slice(2)) {
  const options = parseArguments(args);
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
    localInstallBuildArguments(),
    { env: buildEnv },
  );

  const appPath = resolve(
    desktopDir,
    "src-tauri/target/debug/bundle/macos/Voice Flow Dev.app",
  );
  verifyBundle(appPath);
  console.log(`Verified local development application: ${appPath}`);

  if (options.open) {
    runRequired("/usr/bin/open", [appPath]);
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main();
}
