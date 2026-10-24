import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";
import { Dashboard } from "../Dashboard";
import type { RecordingStateEvent, TranscriptionPartialEvent } from "@/lib/tauri";

const liveEvents = vi.hoisted(() => ({
  recording: vi.fn(async (_callback: (event: RecordingStateEvent) => void) => () => undefined),
  partial: vi.fn(async (_callback: (event: TranscriptionPartialEvent) => void) => () => undefined),
}));

const { getHome, copyEntry, getHistory } = vi.hoisted(() => ({ getHome: vi.fn(), copyEntry: vi.fn(), getHistory: vi.fn() }));
vi.mock("@/lib/tauri", () => ({
  homeCommands: { getSnapshot: getHome },
  statisticsCommands: { getStatistics: vi.fn(async () => ({ period: "7d", wordCount: 0, dictationCount: 0, audioDurationMs: 0, activeDays: 0, trend: [] })) },
  historyCommands: { copyEntry, getHistory },
  events: { onRecordingStateChanged: liveEvents.recording, onTranscriptionPartial: liveEvents.partial, onTranscriptionComplete: vi.fn(async () => () => undefined), onTranscriptionError: vi.fn(async () => () => undefined), onModelDownloadComplete: vi.fn(async () => () => undefined), onModelDeleted: vi.fn(async () => () => undefined) },
}));
vi.mock("@/contexts/SettingsContext", () => ({ useSettingsContext: () => ({ settings: null }) }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("@/i18n", () => ({ default: { language: "en" }, supportedLanguages: [] }));
vi.mock("@/components/ui/confirm", () => ({ useConfirm: () => vi.fn() }));
vi.mock("@/lib/toast", () => ({ showErrorToast: vi.fn(), showToast: vi.fn() }));

beforeEach(() => { vi.clearAllMocks(); copyEntry.mockResolvedValue(undefined); getHistory.mockResolvedValue([]); });

it("renders live replacements without saving or replacing completed history", async () => {
  getHome.mockResolvedValue({ readiness: "ready", setup_path: null, hotkey: "Fn", trigger_mode: "hold", is_cloud: false, last_result: null });
  getHistory.mockResolvedValue([{ id: "retained", created_at: 1700000000000, raw_text: "Retained result", final_text: "Retained result", status: "success", source_kind: "recording", stt_engine: "whisper", delivery_status: "copied" }]);
  render(<MemoryRouter><Dashboard /></MemoryRouter>);
  expect(await screen.findByText("Retained result")).toBeInTheDocument();
  expect(liveEvents.recording).toHaveBeenCalled();
  act(() => {
    liveEvents.recording.mock.lastCall?.[0]({ task_id: 10, status: "recording" });
    liveEvents.partial.mock.lastCall?.[0]({ task_id: 10, text: "Bonjour", is_definite: false });
  });
  expect(screen.getByText("Bonjour")).toHaveAttribute("role", "status");
  act(() => liveEvents.partial.mock.lastCall?.[0]({ task_id: 10, text: "Bonjour le monde", is_definite: false }));
  expect(screen.queryByText("Bonjour", { exact: true })).not.toBeInTheDocument();
  expect(screen.getByText("Bonjour le monde")).toBeInTheDocument();
  expect(screen.getByText("Retained result")).toBeInTheDocument();
  expect(getHistory).toHaveBeenCalledTimes(1);
  act(() => liveEvents.recording.mock.lastCall?.[0]({ task_id: 10, status: "idle" }));
  expect(screen.queryByText("Bonjour le monde")).not.toBeInTheDocument();
});

it("renders an authorized partial after mounting during recording without a snapshot", async () => {
  getHome.mockResolvedValue({ readiness: "ready", setup_path: null, hotkey: "Fn", trigger_mode: "hold", is_cloud: false, last_result: null });
  render(<MemoryRouter><Dashboard /></MemoryRouter>);
  await waitFor(() => expect(liveEvents.partial).toHaveBeenCalled());
  act(() => liveEvents.partial.mock.lastCall?.[0]({ task_id: 15, text: "Already recording", is_definite: false }));
  expect(screen.getByText("Already recording")).toBeInTheDocument();
  act(() => liveEvents.recording.mock.lastCall?.[0]({ task_id: 15, status: "transcribing" }));
  act(() => liveEvents.partial.mock.lastCall?.[0]({ task_id: 15, text: "Late preview", is_definite: false }));
  expect(screen.queryByText("Already recording")).not.toBeInTheDocument();
  expect(screen.queryByText("Late preview")).not.toBeInTheDocument();
});

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
