export type ThemeMode = "system" | "light" | "dark";

const RESOLVED_THEME_STORAGE_KEY = "voiceflow-theme";
const LEGACY_RESOLVED_THEME_STORAGE_KEY = ["aria", "type-theme"].join("");

function getStoredResolvedTheme(): "light" | "dark" | null {
  try {
    const currentValue = localStorage.getItem(RESOLVED_THEME_STORAGE_KEY);
    if (currentValue === "light" || currentValue === "dark") {
      return currentValue;
    }

    const legacyValue = localStorage.getItem(LEGACY_RESOLVED_THEME_STORAGE_KEY);
    if (legacyValue === "light" || legacyValue === "dark") {
      localStorage.setItem(RESOLVED_THEME_STORAGE_KEY, legacyValue);
      localStorage.removeItem(LEGACY_RESOLVED_THEME_STORAGE_KEY);
      return legacyValue;
    }

    return null;
  } catch {
    return null;
  }
}

export function getSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function resolveThemeMode(mode: ThemeMode): "light" | "dark" {
  return mode === "system" ? getSystemTheme() : mode;
}

export function applyResolvedTheme(theme: "light" | "dark") {
  document.documentElement.classList.toggle("dark", theme === "dark");
  try {
    localStorage.setItem(RESOLVED_THEME_STORAGE_KEY, theme);
  } catch {
    // Ignore storage failures and still apply the DOM class.
  }
}

export function applyTheme(mode: ThemeMode) {
  applyResolvedTheme(resolveThemeMode(mode));
}

export function applyInitialTheme() {
  const storedTheme = getStoredResolvedTheme();
  applyResolvedTheme(storedTheme ?? getSystemTheme());
}
