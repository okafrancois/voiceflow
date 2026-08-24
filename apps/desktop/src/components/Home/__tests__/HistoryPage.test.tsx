import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/ui/confirm";
import type { FileTranscriptionJob } from "@/lib/tauri";
import { HistoryPage } from "../HistoryPage";

const {
  getHistoryMock,
  getHistoryCountMock,
  selectMediaFileMock,
  startFileJobMock,
  getFileJobMock,
  listFileJobsMock,
  cancelFileJobMock,
  onFileJobChangedMock,
  repolishEntryMock,
  copyEntryMock,
  selectExportFileMock,
  exportEntryMock,
  getAudioMock,
  showErrorToastMock,
} = vi.hoisted(() => ({
  getHistoryMock: vi.fn(),
  getHistoryCountMock: vi.fn(),
  selectMediaFileMock: vi.fn(),
  startFileJobMock: vi.fn(),
  getFileJobMock: vi.fn(),
  listFileJobsMock: vi.fn(),
  cancelFileJobMock: vi.fn(),
  onFileJobChangedMock: vi.fn(),
  repolishEntryMock: vi.fn(),
  copyEntryMock: vi.fn(),
  selectExportFileMock: vi.fn(),
  exportEntryMock: vi.fn(),
  getAudioMock: vi.fn(),
  showErrorToastMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    onDragDropEvent: vi.fn(async () => vi.fn()),
  }),
}));

vi.mock("@/i18n", () => ({
  supportedLanguages: [
    { code: "en", name: "English" },
    { code: "fr", name: "French" },
  ],
}));

vi.mock("@/lib/tauri", () => ({
  events: {
    onRetryStateChanged: vi.fn(async () => vi.fn()),
    onRetryComplete: vi.fn(async () => vi.fn()),
    onRetryError: vi.fn(async () => vi.fn()),
    onFileTranscriptionJobChanged: onFileJobChangedMock,
  },
  historyCommands: {
    getHistory: getHistoryMock,
    getHistoryCount: getHistoryCountMock,
    selectMediaFile: selectMediaFileMock,
    startFileJob: startFileJobMock,
    getFileJob: getFileJobMock,
    listFileJobs: listFileJobsMock,
    cancelFileJob: cancelFileJobMock,
    retryTranscription: vi.fn(),
    retranscribeEntry: vi.fn(),
    repolishEntry: repolishEntryMock,
    selectExportFile: selectExportFileMock,
    exportEntry: exportEntryMock,
    getAudio: getAudioMock,
    copyEntry: copyEntryMock,
    reinsertEntry: vi.fn(),
    deleteEntry: vi.fn(),
    clearAll: vi.fn(),
  },
}));

let fileJobListener: ((job: FileTranscriptionJob) => void) | undefined;

vi.mock("@/lib/logger", () => ({
  logger: { info: vi.fn(), error: vi.fn() },
}));

vi.mock("@/lib/toast", () => ({
  showToast: vi.fn(),
  showErrorToast: showErrorToastMock,
  showInfoToast: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

const entry = {
  id: "entry-1",
  created_at: 1_700_000_000_000,
  raw_text: "raw words",
  final_text: "Final words.",
  stt_engine: "whisper",
  stt_model: "whisper-base",
  language: "en",
  audio_duration_ms: 2_000,
  stt_duration_ms: 300,
  polish_duration_ms: 40,
  total_duration_ms: 340,
  polish_applied: true,
  polish_engine: "qwen",
  is_cloud: false,
  audio_path: "/tmp/managed.wav",
  status: "success",
  error: null,
  source_kind: "file",
  source_path: "/tmp/source.wav",
  translation_target: null,
  timed_segments: [],
  delivery_status: "not_delivered",
};

function renderPage() {
  return render(
    <ConfirmProvider>
      <HistoryPage />
    </ConfirmProvider>,
  );
}

describe("HistoryPage workbench", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getHistoryMock.mockResolvedValue([entry]);
    getHistoryCountMock.mockResolvedValue(1);
    selectMediaFileMock.mockResolvedValue("/tmp/import.wav");
    const request = { path: "/tmp/import.wav", profile_id: null, translation_target: null };
    const queuedJob = {
      id: "job-1",
      state: "queued",
      progress_percent: 0,
      request,
      result: null,
      error: null,
    } satisfies FileTranscriptionJob;
    startFileJobMock.mockResolvedValue(queuedJob);
    getFileJobMock.mockResolvedValue(queuedJob);
    listFileJobsMock.mockResolvedValue([]);
    cancelFileJobMock.mockResolvedValue({ id: "job-1", state: "canceled", progress_percent: 0, request, result: null, error: null });
    onFileJobChangedMock.mockImplementation(async (listener) => {
      fileJobListener = listener;
      return vi.fn();
    });
    repolishEntryMock.mockResolvedValue(entry);
    copyEntryMock.mockResolvedValue(undefined);
    selectExportFileMock.mockResolvedValue(null);
    exportEntryMock.mockResolvedValue("/tmp/transcript.txt");
    getAudioMock.mockRejectedValue("Video playback is unavailable");
  });

  it("imports a selected media file and refreshes history", async () => {
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "history.workbench.choose" }));

    await waitFor(() => {
      expect(startFileJobMock).toHaveBeenCalledWith({
        path: "/tmp/import.wav",
        profile_id: null,
        translation_target: null,
      });
    });
    await act(async () => {
      fileJobListener?.({
        id: "job-1",
        state: "completed",
        progress_percent: 100,
        request: { path: "/tmp/import.wav", profile_id: null, translation_target: null },
        result: null,
        error: null,
      });
    });
    expect((await screen.findAllByText("history.workbench.completed")).length).toBeGreaterThan(0);
    expect(getHistoryMock.mock.calls.length).toBeGreaterThan(1);
  });

  it("shows raw and final text and copies either version", async () => {
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "history.actions.details" }));

    const card = screen.getByTestId("history-entry-entry-1");
    expect(within(card).getByText("raw words")).toBeInTheDocument();
    expect(within(card).getAllByText("Final words.")).toHaveLength(2);
    expect(within(card).getByText("history.delivery.notDelivered")).toBeInTheDocument();
    expect(within(card).getByRole("option", { name: "history.translation.none" })).toHaveValue("");
    fireEvent.click(within(card).getByRole("button", { name: "history.actions.copyRaw" }));
    await waitFor(() => expect(copyEntryMock).toHaveBeenCalledWith("entry-1", "raw"));
  });

  it("runs explicit translation with the selected target", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "history.actions.details" }));
    const card = screen.getByTestId("history-entry-entry-1");
    fireEvent.change(within(card).getByLabelText("history.translation.target"), {
      target: { value: "fr" },
    });
    fireEvent.click(within(card).getByRole("button", { name: "history.actions.translate" }));

    await waitFor(() => {
      expect(repolishEntryMock).toHaveBeenCalledWith("entry-1", null, "fr");
    });
  });

  it("cancels an active import through the backend job", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "history.workbench.choose" }));

    fireEvent.click(await screen.findByRole("button", { name: "history.workbench.cancel" }));

    await waitFor(() => expect(cancelFileJobMock).toHaveBeenCalledWith("job-1"));
    expect((await screen.findAllByText("history.workbench.canceled")).length).toBeGreaterThan(0);
  });

  it("restores the backend job list and can cancel a listed job", async () => {
    const request = { path: "/tmp/previous.wav", profile_id: null, translation_target: null };
    listFileJobsMock.mockResolvedValue([{ id: "job-previous", state: "running", progress_percent: 5, request, result: null, error: null }]);
    cancelFileJobMock.mockResolvedValue({ id: "job-previous", state: "canceled", progress_percent: 5, request, result: null, error: null });
    renderPage();

    const list = await screen.findByTestId("media-job-list");
    expect(within(list).getByText("/tmp/previous.wav")).toBeInTheDocument();
    fireEvent.click(within(list).getByRole("button", { name: "history.workbench.cancel" }));

    await waitFor(() => expect(cancelFileJobMock).toHaveBeenCalledWith("job-previous"));
  });

  it("requires explicit confirmation before overwriting an export", async () => {
    selectExportFileMock.mockResolvedValue("/tmp/transcript.txt");
    exportEntryMock
      .mockRejectedValueOnce("Export destination exists and overwrite was not confirmed")
      .mockResolvedValueOnce("/tmp/transcript.txt");
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "history.actions.details" }));
    const card = screen.getByTestId("history-entry-entry-1");

    fireEvent.click(within(card).getByRole("button", { name: "TXT" }));
    fireEvent.click(await screen.findByRole("button", { name: "history.export.overwrite" }));

    await waitFor(() => {
      expect(exportEntryMock).toHaveBeenNthCalledWith(1, "entry-1", "txt", "/tmp/transcript.txt", false);
      expect(exportEntryMock).toHaveBeenNthCalledWith(2, "entry-1", "txt", "/tmp/transcript.txt", true);
    });
  });

  it("reports media that the backend refuses to load for playback", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "history.actions.details" }));
    const card = screen.getByTestId("history-entry-entry-1");

    fireEvent.click(within(card).getByRole("button", { name: "history.actions.playAudio" }));

    await waitFor(() => expect(showErrorToastMock).toHaveBeenCalledWith("Video playback is unavailable"));
    expect(within(card).queryByRole("audio")).not.toBeInTheDocument();
  });
});
