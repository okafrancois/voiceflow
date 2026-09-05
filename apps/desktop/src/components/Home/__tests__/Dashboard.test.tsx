import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";
import { Dashboard } from "../Dashboard";

const { getHome, copyEntry, getHistory } = vi.hoisted(() => ({ getHome: vi.fn(), copyEntry: vi.fn(), getHistory: vi.fn() }));
vi.mock("@/lib/tauri", () => ({
  homeCommands: { getSnapshot: getHome },
  statisticsCommands: { getStatistics: vi.fn(async () => ({ period: "7d", wordCount: 0, dictationCount: 0, audioDurationMs: 0, activeDays: 0, trend: [] })) },
  historyCommands: { copyEntry, getHistory },
  events: { onTranscriptionComplete: vi.fn(async () => () => undefined), onTranscriptionError: vi.fn(async () => () => undefined), onModelDownloadComplete: vi.fn(async () => () => undefined), onModelDeleted: vi.fn(async () => () => undefined) },
}));
vi.mock("@/contexts/SettingsContext", () => ({ useSettingsContext: () => ({ settings: null }) }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("@/i18n", () => ({ default: { language: "en" }, supportedLanguages: [] }));
vi.mock("@/components/ui/confirm", () => ({ useConfirm: () => vi.fn() }));
vi.mock("@/lib/toast", () => ({ showErrorToast: vi.fn(), showToast: vi.fn() }));

beforeEach(() => { vi.clearAllMocks(); copyEntry.mockResolvedValue(undefined); getHistory.mockResolvedValue([]); });

it("shows backend setup guidance without declaring an unavailable microphone ready", async () => {
  getHome.mockResolvedValue({ readiness: "microphone_required", setup_path: "/permission", hotkey: "Cmd+Slash", trigger_mode: "hold", is_cloud: false, last_result: null });
  render(<MemoryRouter><Dashboard /></MemoryRouter>);
  expect(await screen.findByText("home.readiness.microphone_required")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "home.openSetup" })).toHaveAttribute("href", "/permission");
  expect(screen.queryByText("home.readiness.ready")).not.toBeInTheDocument();
  expect(screen.queryByText("dashboard.dictation.savedTime")).not.toBeInTheDocument();
});

it("keeps a failed delivery recoverable through its exact history entry", async () => {
  getHome.mockResolvedValue({ readiness: "ready", setup_path: null, hotkey: "Cmd+Slash", trigger_mode: "hold", is_cloud: false, last_result: { id: "entry-42", raw_text: "Raw words", final_text: "Final words", can_copy_raw: true, can_copy_final: true, delivery_failed: true, error: null } });
  getHistory.mockResolvedValue([{ id: "entry-42", created_at: 1700000000000, raw_text: "Raw words", final_text: "Final words", status: "success", source_kind: "recording", stt_engine: "whisper", delivery_status: "failed" }]);
  render(<MemoryRouter><Dashboard /></MemoryRouter>);
  expect(await screen.findByText("Final words")).toBeInTheDocument();
  expect(screen.getByText("home.deliveryFailed")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "history.actions.details" }));
  fireEvent.click(screen.getByRole("button", { name: "history.actions.copyFinal" }));
  await waitFor(() => expect(copyEntry).toHaveBeenCalledWith("entry-42", "final"));
});

 it("shows recent history and statistics access while setup is incomplete", async () => {
  getHome.mockResolvedValue({ readiness: "microphone_required", setup_path: "/permission", hotkey: "Fn", trigger_mode: "hold", is_cloud: false, last_result: null });
  getHistory.mockResolvedValue([{ id: "recent", created_at: 1700000000000, raw_text: "A retained dictation", final_text: "A retained dictation", status: "success", source_kind: "recording", stt_engine: "whisper", delivery_status: "copied" }]);
  render(<MemoryRouter><Dashboard /></MemoryRouter>);
  expect(await screen.findByText("A retained dictation")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "usage.openStatistics" })).toHaveAttribute("href", "/statistics");
  expect(getHistory).toHaveBeenCalledWith({ limit: 5, offset: 0 });
});
