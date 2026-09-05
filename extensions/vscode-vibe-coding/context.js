"use strict";

const MAX_IDENTIFIERS = 64;
const MAX_IDENTIFIER_CHARS = 128;

function flattenSymbolNames(symbols) {
  const names = [];
  const visit = (symbol) => {
    if (typeof symbol?.name === "string") names.push(symbol.name);
    if (Array.isArray(symbol?.children)) symbol.children.forEach(visit);
  };
  if (Array.isArray(symbols)) symbols.forEach(visit);
  return names;
}

function sanitizeIdentifiers(values) {
  const seen = new Set();
  const identifiers = [];
  for (const value of values) {
    if (typeof value !== "string") continue;
    const cleaned = Array.from(value)
      .filter((character) => !/[\u0000-\u001f\u007f]/u.test(character))
      .slice(0, MAX_IDENTIFIER_CHARS)
      .join("")
      .trim();
    if (!cleaned || seen.has(cleaned)) continue;
    seen.add(cleaned);
    identifiers.push(cleaned);
    if (identifiers.length === MAX_IDENTIFIERS) break;
  }
  return identifiers;
}

function buildContextPayload({
  language,
  filePath,
  symbol,
  editorId,
  workspace,
  documentSymbols,
}) {
  return {
    language: language || null,
    file_path: filePath || null,
    symbol: symbol || null,
    editor_id: editorId || null,
    workspace: workspace || null,
    identifiers: sanitizeIdentifiers([
      symbol,
      ...flattenSymbolNames(documentSymbols),
    ]),
  };
}

module.exports = {
  MAX_IDENTIFIERS,
  MAX_IDENTIFIER_CHARS,
  buildContextPayload,
  flattenSymbolNames,
  sanitizeIdentifiers,
};
