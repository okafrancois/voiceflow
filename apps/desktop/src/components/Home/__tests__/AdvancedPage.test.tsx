import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { AdvancedPage } from "../AdvancedPage";

const { updateSetting } = vi.hoisted(() => ({ updateSetting: vi.fn(async () => undefined) }));
vi.mock("@/contexts/SettingsContext", () => ({ useSettingsContext: () => ({ settings: { developer_bridge_enabled: false }, updateSetting }) }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("@/i18n", () => ({ default: { language: "en" } }));
vi.mock("@/lib/toast", () => ({ showErrorToast: vi.fn() }));

it("keeps optional tools reachable and requests bridge activation through backend settings", async () => {
  render(<MemoryRouter><AdvancedPage /></MemoryRouter>);
  expect(screen.getByRole("link", { name: "advanced.profiles.title" })).toHaveAttribute("href", "/workflows?tab=profiles");
  expect(screen.getByRole("link", { name: "advanced.media.title" })).toHaveAttribute("href", "/workbench");
  expect(screen.getByRole("link", { name: "advanced.diagnostics.title" })).toHaveAttribute("href", "/quality");
  const toggle = screen.getByRole("switch", { name: "advanced.bridge.enable" });
  expect(toggle).not.toBeChecked();
  fireEvent.click(toggle);
  await waitFor(() => expect(updateSetting).toHaveBeenCalledWith("developer_bridge_enabled", true));
  expect(toggle).not.toBeChecked();
});
