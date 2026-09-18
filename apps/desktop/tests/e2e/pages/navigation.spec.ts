import { test, expect } from "../fixtures";
import {
  clearTranscriptionHistory,
  disableAutoSnapshot,
  openRouteWithOnboarding,
} from "../utils/helpers";

const deprecatedSidebarRoutes = [
  "/settings",
  "/hotkey",
  "/private-ai",
  "/cloud",
  "/permission",
  "/logs",
  "/changelog",
  "/polish-templates",
  "/workflows",
  "/quality",
];

test("Sidebar exposes only current primary navigation", async ({
  tauriPage,
}, testInfo) => {
  disableAutoSnapshot(testInfo);
  await openRouteWithOnboarding(tauriPage, "/");
  await clearTranscriptionHistory(tauriPage);

  const sidebar = tauriPage.locator('[data-testid="home-sidebar"]');
  await expect(sidebar).toBeVisible({ timeout: 15000 });

  for (const label of [
    "Dictation",
    "Statistics",
    "History",
    "Dictionary",
    "Snippets",
    "Styles",
    "Vibe coding",
    "Advanced",
    "Settings",
    "About",
  ]) {
    await expect(sidebar.getByText(label, { exact: true })).toBeVisible();
  }

  const sidebarHrefs = await tauriPage.evaluate<string[]>(
    `Array.from(document.querySelectorAll('[data-testid="home-sidebar"] a'))
      .map((link) => link.getAttribute('href'))
      .filter((href) => href !== null)`,
  );
  for (const route of [
    "/",
    "/statistics",
    "/history",
    "/dictionary",
    "/snippets",
    "/styles",
    "/vibe-coding",
    "/advanced",
    "/about",
  ]) {
    expect(sidebarHrefs).toContain(route);
  }
  for (const route of deprecatedSidebarRoutes) {
    expect(sidebarHrefs).not.toContain(route);
  }

  await expect(
    tauriPage.locator('[data-testid="dictation-home"]'),
  ).toBeVisible();

  await sidebar.locator('a[href="/history"]').click();
  const historyPage = tauriPage.locator('[data-testid="history-page"]');
  await expect(historyPage).toBeVisible({ timeout: 10000 });
  await expect(
    historyPage.getByPlaceholder("Search transcriptions..."),
  ).toBeVisible();

  await sidebar.locator('a[href="/dictionary"]').click();
  await expect(
    tauriPage.locator('[data-testid="dictionary-page"]'),
  ).toBeVisible({ timeout: 10000 });

  await sidebar.locator('a[href="/snippets"]').click();
  await expect(tauriPage.locator('[data-testid="snippets-page"]')).toBeVisible({
    timeout: 10000,
  });

  await sidebar.locator('a[href="/styles"]').click();
  await expect(tauriPage.locator('[data-testid="styles-page"]')).toBeVisible({
    timeout: 10000,
  });

  await sidebar.locator('a[href="/advanced"]').click();
  await expect(tauriPage.locator('[data-testid="advanced-page"]')).toBeVisible({
    timeout: 10000,
  });

  await sidebar.locator('[data-testid="nav-about"]').click();
  const aboutPage = tauriPage.locator('[data-testid="about-page"]');
  await expect(aboutPage).toBeVisible({ timeout: 10000 });
  await expect(aboutPage.locator("h1")).toContainText("Voice Flow");
  await expect(aboutPage.getByText("Software Updates")).toBeVisible();
  await expect(aboutPage.getByText("Supported Platforms")).toBeVisible();

  await aboutPage.getByText("View Changelog").click();
  await expect(tauriPage.locator('[data-testid="changelog-page"]')).toBeVisible(
    { timeout: 10000 },
  );
  await expect(tauriPage.locator("h1")).toContainText("Changelog");

  await sidebar.locator('a[href="/"]').click();
  await expect(tauriPage.locator('[data-testid="dictation-home"]')).toBeVisible(
    { timeout: 10000 },
  );
});
