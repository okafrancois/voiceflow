"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const {
  MAX_IDENTIFIERS,
  buildContextPayload,
  flattenSymbolNames,
  sanitizeIdentifiers,
} = require("../context");

test("collects nested document symbols without reading document text", () => {
  const symbols = [
    { name: "HTTPServer", children: [{ name: "handleRequest", children: [] }] },
    { name: "VoiceFlowConfig" },
  ];

  assert.deepEqual(flattenSymbolNames(symbols), [
    "HTTPServer",
    "handleRequest",
    "VoiceFlowConfig",
  ]);
  assert.deepEqual(
    buildContextPayload({
      language: "typescriptreact",
      filePath: "src/App.tsx",
      symbol: "HTTPServer",
      editorId: "Cursor",
      workspace: "voiceflow",
      documentSymbols: symbols,
    }),
    {
      language: "typescriptreact",
      file_path: "src/App.tsx",
      symbol: "HTTPServer",
      editor_id: "Cursor",
      workspace: "voiceflow",
      identifiers: ["HTTPServer", "handleRequest", "VoiceFlowConfig"],
    },
  );
});

test("identifier payload is deduplicated and bounded", () => {
  const values = Array.from({ length: 80 }, (_, index) => `name_${index}`);
  values.unshift("name_0", "bad\nname");
  const identifiers = sanitizeIdentifiers(values);

  assert.equal(identifiers.length, MAX_IDENTIFIERS);
  assert.equal(identifiers[0], "name_0");
  assert.equal(identifiers[1], "badname");
  assert.equal(new Set(identifiers).size, identifiers.length);
});
