"use strict";

const assert = require("node:assert/strict");
const { EventEmitter } = require("node:events");
const test = require("node:test");
const { createExtension, resolveCliPath, runCli } = require("../extension");

function deferred() {
  let resolve;
  const promise = new Promise((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

function fakeSpawn(calls, stdinError) {
  return (_command, args) => {
    const child = new EventEmitter();
    child.stdout = new EventEmitter();
    child.stderr = new EventEmitter();
    child.stdout.setEncoding = () => {};
    child.stderr.setEncoding = () => {};
    child.stdin = new EventEmitter();
    child.stdin.end = (input) => {
      calls.push({ args: args.slice(1), input });
      if (stdinError) {
        queueMicrotask(() => child.stdin.emit("error", stdinError));
      } else {
        queueMicrotask(() => child.emit("close", 0));
      }
    };
    child.kill = () => {};
    return child;
  };
}

function harness(symbolResult, initiallyEnabled = false) {
  const commands = new Map();
  const windowStateListeners = [];
  const editorListeners = [];
  const documentListeners = [];
  const values = new Map([
    ["voiceFlowVibe.contextSharingEnabled", initiallyEnabled],
  ]);
  const document = {
    languageId: "typescriptreact",
    uri: { scheme: "file", fsPath: "/workspace/src/App.tsx" },
  };
  const vscode = {
    env: { appName: "Cursor" },
    commands: {
      registerCommand(name, handler) {
        commands.set(name, handler);
        return { dispose() {} };
      },
      executeCommand: () => symbolResult.promise,
    },
    window: {
      activeTextEditor: { document, selection: { active: {} } },
      state: { focused: true },
      onDidChangeActiveTextEditor(listener) {
        editorListeners.push(listener);
        return { dispose() {} };
      },
      onDidChangeWindowState(listener) {
        windowStateListeners.push(listener);
        return { dispose() {} };
      },
      showErrorMessage() {},
      showInformationMessage() {},
    },
    workspace: {
      getConfiguration: () => ({ get: () => "/tmp/voiceflow" }),
      getWorkspaceFolder: () => ({
        name: "workspace",
        uri: { fsPath: "/workspace" },
      }),
      onDidChangeTextDocument(listener) {
        documentListeners.push(listener);
        return { dispose() {} };
      },
    },
  };
  const extensionContext = {
    globalState: {
      get: (key, fallback) => values.get(key) ?? fallback,
      update: async (key, value) => values.set(key, value),
    },
    subscriptions: [],
  };
  return { commands, document, extensionContext, values, vscode, windowStateListeners };
}

function disposeAll(extensionContext) {
  for (const disposable of extensionContext.subscriptions) disposable.dispose?.();
}

test("a stale symbol response cannot publish context after disable", async () => {
  const symbols = deferred();
  const calls = [];
  const testHarness = harness(symbols);
  createExtension(testHarness.vscode, fakeSpawn(calls))(testHarness.extensionContext);

  const enabling = testHarness.commands.get("voiceFlowVibe.enable")();
  await new Promise((resolve) => setImmediate(resolve));
  const disabling = testHarness.commands.get("voiceFlowVibe.disable")();
  symbols.resolve([{ name: "HTTPServer" }]);
  await Promise.all([enabling, disabling]);

  assert.deepEqual(calls.map((call) => call.args), [
    ["vibe-coding", "on"],
    ["vibe-coding", "off"],
  ]);
  assert.equal(testHarness.values.get("voiceFlowVibe.contextSharingEnabled"), false);
  disposeAll(testHarness.extensionContext);
});

test("a stale symbol response cannot publish after editor focus leaves", async () => {
  const symbols = deferred();
  const calls = [];
  const testHarness = harness(symbols, true);
  createExtension(testHarness.vscode, fakeSpawn(calls))(testHarness.extensionContext);

  const sending = testHarness.commands.get("voiceFlowVibe.sendContext")();
  testHarness.vscode.window.state.focused = false;
  testHarness.windowStateListeners[0]({ focused: false });
  symbols.resolve([{ name: "HTTPServer" }]);
  await sending;
  await new Promise((resolve) => setImmediate(resolve));

  assert.deepEqual(calls.map((call) => call.args), [["clear-code-context"]]);
  disposeAll(testHarness.extensionContext);
});

test("disable stays local when the CLI cannot turn the backend off", async () => {
  const symbols = deferred();
  symbols.resolve([]);
  const calls = [];
  const testHarness = harness(symbols, true);
  const error = new Error("write EPIPE");
  createExtension(testHarness.vscode, fakeSpawn(calls, error))(testHarness.extensionContext);

  await testHarness.commands.get("voiceFlowVibe.disable")();

  assert.equal(testHarness.values.get("voiceFlowVibe.contextSharingEnabled"), false);
  disposeAll(testHarness.extensionContext);
});

test("configured CLI paths must be absolute application paths", async () => {
  const symbols = deferred();
  const testHarness = harness(symbols);
  testHarness.vscode.workspace.getConfiguration = () => ({ get: () => "./voiceflow" });
  assert.throws(() => resolveCliPath(testHarness.vscode), /absolute application path/);

  await assert.rejects(
    runCli(testHarness.vscode, fakeSpawn([], new Error("write EPIPE")), ["status"]),
    /absolute application path/,
  );
});
