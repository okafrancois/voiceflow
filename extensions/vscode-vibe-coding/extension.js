"use strict";

const { spawn } = require("node:child_process");
const { existsSync } = require("node:fs");
const path = require("node:path");
const { buildContextPayload } = require("./context");

const ENABLED_KEY = "voiceFlowVibe.contextSharingEnabled";
const REFRESH_MS = 60_000;

function resolveCliPath(vscode, fileExists = existsSync, environment = process.env) {
  const configured = vscode.workspace
    .getConfiguration("voiceFlowVibe")
    .get("cliPath", "")
    .trim();
  if (configured) {
    if (!path.isAbsolute(configured)) {
      throw new Error("voiceFlowVibe.cliPath must be an absolute application path");
    }
    return configured;
  }
  const candidates = [
    path.join(
      environment.HOME || "",
      "Applications/Voice Flow Dev.app/Contents/MacOS/voiceflow",
    ),
    "/Applications/Voice Flow.app/Contents/MacOS/voiceflow",
  ];
  return candidates.find(fileExists) || "voiceflow";
}

function runCli(vscode, spawnProcess, args, input) {
  return new Promise((resolve, reject) => {
    let settled = false;
    let timeout;
    const finish = (callback) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      callback();
    };
    let child;
    try {
      child = spawnProcess(resolveCliPath(vscode), ["--cli", ...args], {
        stdio: ["pipe", "pipe", "pipe"],
        windowsHide: true,
      });
    } catch (error) {
      reject(error);
      return;
    }
    let stdout = "";
    let stderr = "";
    timeout = setTimeout(() => {
      child.kill();
      finish(() => reject(new Error("Voice Flow CLI timed out")));
    }, 5_000);
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.stdin.on("error", (error) => {
      finish(() => reject(error));
    });
    child.on("error", (error) => {
      finish(() => reject(error));
    });
    child.on("close", (code) => {
      finish(() => {
        if (code === 0) resolve(stdout);
        else reject(new Error(stderr.trim() || `Voice Flow CLI exited with ${code}`));
      });
    });
    child.stdin.end(input);
  });
}

function symbolAtPosition(symbols, position) {
  if (!Array.isArray(symbols)) return undefined;
  for (const symbol of symbols) {
    const range = symbol.range || symbol.location?.range;
    if (range?.contains(position)) {
      return symbolAtPosition(symbol.children, position) || symbol.name;
    }
  }
  return undefined;
}

async function currentContext(vscode) {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.uri.scheme !== "file") return undefined;
  const document = editor.document;
  const folder = vscode.workspace.getWorkspaceFolder(document.uri);
  const filePath = folder
    ? path.relative(folder.uri.fsPath, document.uri.fsPath)
    : path.basename(document.uri.fsPath);
  const symbols = await vscode.commands.executeCommand(
    "vscode.executeDocumentSymbolProvider",
    document.uri,
  );
  return {
    document,
    payload: buildContextPayload({
      language: document.languageId,
      filePath,
      symbol: symbolAtPosition(symbols, editor.selection.active),
      editorId: vscode.env.appName,
      workspace: folder?.name,
      documentSymbols: symbols,
    }),
  };
}

function createExtension(vscode, spawnProcess = spawn) {
  return function activateExtension(extensionContext) {
    let refreshTimer;
    let refreshPending;
    let generation = 0;
    let disposed = false;
    const enabled = () => extensionContext.globalState.get(ENABLED_KEY, false);
    const reportError = (error) =>
      vscode.window.showErrorMessage(`Voice Flow Vibe Coding: ${error.message}`);
    const invokeCli = (args, input) => runCli(vscode, spawnProcess, args, input);
    const invalidatePending = () => {
      generation += 1;
      clearTimeout(refreshPending);
      refreshPending = undefined;
    };
    const publishCurrentContext = async () => {
      const requestGeneration = ++generation;
      const captured = await currentContext(vscode);
      if (
        disposed ||
        requestGeneration !== generation ||
        !enabled() ||
        !vscode.window.state.focused ||
        captured?.document !== vscode.window.activeTextEditor?.document
      ) {
        return false;
      }
      if (!captured) {
        await invokeCli(["clear-code-context"]);
        return false;
      }
      await invokeCli(["code-context"], JSON.stringify(captured.payload));
      return true;
    };
    const refresh = () => {
      invalidatePending();
      if (!enabled() || !vscode.window.state.focused || disposed) return;
      refreshPending = setTimeout(
        () => void publishCurrentContext().catch(reportError),
        250,
      );
    };

    extensionContext.subscriptions.push(
      vscode.commands.registerCommand("voiceFlowVibe.enable", async () => {
        try {
          await invokeCli(["vibe-coding", "on"]);
          await extensionContext.globalState.update(ENABLED_KEY, true);
          const sent = await publishCurrentContext();
          if (!enabled()) return;
          vscode.window.showInformationMessage(
            sent
              ? "Voice Flow Vibe Coding is sharing active editor metadata."
              : "Voice Flow Vibe Coding is enabled and waiting for a file editor.",
          );
        } catch (error) {
          reportError(error);
        }
      }),
      vscode.commands.registerCommand("voiceFlowVibe.sendContext", async () => {
        try {
          const sent = await publishCurrentContext();
          vscode.window.showInformationMessage(
            sent ? "Voice Flow coding context updated." : "No active file editor was found.",
          );
        } catch (error) {
          reportError(error);
        }
      }),
      vscode.commands.registerCommand("voiceFlowVibe.disable", async () => {
        invalidatePending();
        await extensionContext.globalState.update(ENABLED_KEY, false);
        try {
          await invokeCli(["vibe-coding", "off"]);
          vscode.window.showInformationMessage("Voice Flow Vibe Coding is disabled.");
        } catch (error) {
          reportError(error);
        }
      }),
      vscode.window.onDidChangeActiveTextEditor(refresh),
      vscode.workspace.onDidChangeTextDocument((event) => {
        if (event.document === vscode.window.activeTextEditor?.document) refresh();
      }),
      vscode.window.onDidChangeWindowState((state) => {
        invalidatePending();
        if (enabled() && !state.focused) {
          void invokeCli(["clear-code-context"]).catch(reportError);
        } else {
          refresh();
        }
      }),
    );

    refreshTimer = setInterval(refresh, REFRESH_MS);
    extensionContext.subscriptions.push({
      dispose: () => {
        disposed = true;
        invalidatePending();
        clearInterval(refreshTimer);
      },
    });
    refresh();
  };
}

function activate(extensionContext) {
  return createExtension(require("vscode"))(extensionContext);
}

module.exports = { activate, createExtension, resolveCliPath, runCli };
