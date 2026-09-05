# Voice Flow Vibe Coding editor adapter

This local extension works with VS Code-compatible desktop editors, including
Visual Studio Code, Cursor, and Windsurf. It sends active editor metadata and
names returned by the editor's document-symbol provider to the authenticated
Voice Flow CLI bridge. It does not read or send the document body.

## Install locally

1. In Voice Flow, enable the developer bridge under Advanced. The bridge remains
   off by default.
2. Package the extension:

   ```bash
   cd extensions/vscode-vibe-coding
   npx @vscode/vsce package
   ```

3. In the editor, choose **Extensions: Install from VSIX** and select the created
   `.vsix` file.
4. If Voice Flow is not in a standard macOS location or on `PATH`, set
   `voiceFlowVibe.cliPath` to its executable.
5. Run **Voice Flow: Enable Vibe Coding Context**. This explicitly enables the
   backend mode and context sharing for this editor installation.

The extension refreshes context while its window is focused. It clears context
when focus leaves the editor, and the backend expires unrefreshed context after
five minutes. Disabling Vibe coding clears the backend context.

The payload contains the editor name, language identifier, workspace label,
workspace-relative file path, current symbol, and at most 64 document-symbol
names. It contains no source text. Symbol availability depends on the installed
language extension and its document-symbol provider.

The payload and editor lifecycle are covered by automated tests, and the VSIX
archive is validated after packaging. This release has not been smoke tested in
a live VS Code, Cursor, or Windsurf Extension Development Host.
