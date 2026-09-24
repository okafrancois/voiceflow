#!/usr/bin/env python3
"""Check that every user-facing key has an entry in each Localizable.strings.

Keys are French source strings passed to `L.t(...)` or to the helpers that
translate their arguments (`row`, `settingsCard`, `VFMetric`, ...).

Usage: tools/check-strings.py [--fix-fr]
  --fix-fr  append missing keys to fr.lproj as identity entries.
Exits 1 when an English translation is missing.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent / "VoiceFlow"
SOURCES = ROOT / "Sources" / "VoiceFlow"
CATALOGS = {lang: ROOT / "Resources" / f"{lang}.lproj" / "Localizable.strings" for lang in ("fr", "en")}

LITERAL = r'"((?:[^"\\]|\\.)*)"'
PATTERNS = [
    rf"L\.t\({LITERAL}\)",
    rf"\brow\({LITERAL}",
    rf"\bhelp: {LITERAL}",
    rf"\bsettingsCard\({LITERAL}",
    rf"\bpermissionRow\({LITERAL}",
    rf"\bVFMetric\(\s*label: {LITERAL}",
    rf"\bVFTextField\(placeholder: {LITERAL}",
    rf"\btitle: {LITERAL}",
    rf"\blinkTitle: {LITERAL}",
    rf"\bstepTitle\({LITERAL},\s*{LITERAL}",
    rf"\bengineColumn\({LITERAL}",
]
DEFINITION = re.compile(rf'\(\s*"[a-z]+",\s*{LITERAL},')


def used_keys() -> set[str]:
    keys: set[str] = set()
    for path in SOURCES.glob("*.swift"):
        text = path.read_text()
        for pattern in PATTERNS:
            for match in re.finditer(pattern, text):
                keys.update(group for group in match.groups() if group)
        if path.name == "PolishEngine.swift":
            # Noms des styles de polissage, traduits à l'affichage.
            keys.update(DEFINITION.findall(text))
    # Interpolations are formatted at runtime; they cannot be catalog keys.
    return {key for key in keys if "\\(" not in key and re.search(r"[A-Za-zÀ-ÿ]", key)}


def catalog(path: pathlib.Path) -> dict[str, str]:
    pairs = re.findall(rf"^{LITERAL}\s*=\s*{LITERAL};", path.read_text(), re.MULTILINE)
    return dict(pairs)


def main() -> int:
    keys = used_keys()
    missing = {lang: sorted(keys - catalog(path).keys()) for lang, path in CATALOGS.items()}
    if "--fix-fr" in sys.argv and missing["fr"]:
        with CATALOGS["fr"].open("a") as handle:
            for key in missing["fr"]:
                handle.write(f'"{key}" = "{key}";\n')
        missing["fr"] = []
    for lang, keys_missing in missing.items():
        for key in keys_missing:
            print(f"{lang}: missing \"{key}\"")
    return 1 if missing["en"] else 0


if __name__ == "__main__":
    sys.exit(main())
