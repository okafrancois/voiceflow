import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { VibeCodingPage } from "../VibeCodingPage";
const { getStatus, setEnabled } = vi.hoisted(() => ({ getStatus: vi.fn(), setEnabled: vi.fn() }));
vi.mock("@/lib/tauri", () => ({ vibeCodingCommands: { getStatus, setEnabled }, platformQualityCommands: { clearCodeContext: vi.fn() } }));
vi.mock("@/contexts/SettingsContext", () => ({ useSettingsContext: () => ({ settings: { developer_bridge_enabled: false }, updateSetting: vi.fn() }) }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("@/lib/toast", () => ({ showErrorToast: vi.fn() }));
it("shows backend context and does not pretend enabling succeeded after rejection", async () => {
  getStatus.mockResolvedValue({ enabled: false, state: "disabled", context_active: false, identifiers: [], file_name: null, editor: null, language: null });
  setEnabled.mockRejectedValue(new Error("settings could not be saved"));
  render(<VibeCodingPage />);
  const toggle = await screen.findByRole("switch", { name: "vibe.enable" });
  fireEvent.click(toggle);
  await waitFor(() => expect(setEnabled).toHaveBeenCalledWith(true));
  await waitFor(() => expect(toggle).toBeEnabled());
  expect(toggle).toHaveAttribute("aria-checked", "false");
  expect(screen.getByText("vibe.setupDescription")).toBeInTheDocument();
});
