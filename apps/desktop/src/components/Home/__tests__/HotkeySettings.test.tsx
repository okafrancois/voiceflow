import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { HotkeySettings } from "../HotkeySettings";

const { getTemplatesMock, updateProfileMock } = vi.hoisted(() => ({
  getTemplatesMock: vi.fn(),
  updateProfileMock: vi.fn(),
}));

vi.mock("@/contexts/SettingsContext", () => ({
  useSettingsContext: () => ({
    settings: {
      shortcut_profiles: {
        dictate: {
          hotkey: "Cmd+Slash",
          trigger_mode: "hold",
          action: { Record: { polish_template_id: null } },
        },
        riff: {
          hotkey: "Opt+Slash",
          trigger_mode: "toggle",
          action: { Record: { polish_template_id: "filler" } },
        },
      },
    },
    polishAvailable: true,
  }),
}));

vi.mock("@/lib/tauri", () => ({
  modelCommands: {
    getPolishTemplates: getTemplatesMock,
    getPolishCustomTemplates: vi.fn(async () => []),
  },
  hotkeyCommands: {
    updateProfile: updateProfileMock,
    startCapture: vi.fn(),
    stopCapture: vi.fn(),
    cancelCapture: vi.fn(),
    peekCapture: vi.fn(),
  },
  events: {
    onHotkeyCaptured: vi.fn(async () => () => undefined),
    onShortcutRegistrationFailed: vi.fn(async () => () => undefined),
  },
}));

vi.mock("@/lib/analytics", () => ({
  analytics: { track: vi.fn() },
}));

vi.mock("@/lib/toast", () => ({
  showErrorToast: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

describe("HotkeySettings", () => {
  beforeEach(() => {
    getTemplatesMock.mockResolvedValue([
      { id: "filler", name: "Clean Dictation", description: "", system_prompt: "" },
      { id: "document", name: "Structured Notes", description: "", system_prompt: "" },
    ]);
    updateProfileMock.mockReset();
    updateProfileMock.mockResolvedValue(undefined);
  });

  it("shows one Dictate profile with optional polish templates", async () => {
    render(<HotkeySettings />);

    expect(screen.getByText("Dictate")).toBeInTheDocument();
    expect(screen.queryByText("Riff")).not.toBeInTheDocument();
    expect(screen.queryByText("Custom")).not.toBeInTheDocument();

    const templateSelect = await screen.findByLabelText("Polish Template");
    fireEvent.click(templateSelect);
    expect(screen.getByRole("button", { name: "No Polish" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Structured Notes" }));

    await waitFor(() => {
      expect(updateProfileMock).toHaveBeenCalledWith("dictate", {
        hotkey: "Cmd+Slash",
        trigger_mode: "hold",
        action: { Record: { polish_template_id: "document" } },
      });
    });
  });
});
