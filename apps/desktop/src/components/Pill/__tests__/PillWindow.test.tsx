import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "@/i18n";
import { PillWindow } from "../PillWindow";
import type { AppSettings, PillTooltipEvent, RecordingStateEvent, TranscriptionPartialEvent } from "@/lib/tauri";

type Listener<T> = (event: T) => void;

const liveEvents = vi.hoisted(() => ({
  recording: vi.fn(async (_callback: Listener<RecordingStateEvent>) => () => undefined),
  partial: vi.fn(async (_callback: Listener<TranscriptionPartialEvent>) => () => undefined),
  complete: vi.fn(async () => () => undefined),
}));

const {
  getSettingsMock,
  hidePillMock,
  onPillTooltipMock,
  onSettingsChangedMock,
  listenMock,
  showPillMock,
} = vi.hoisted(() => {
  const tooltipListeners = new Set<Listener<PillTooltipEvent>>();
  const recordingStateListeners = new Set<Listener<{ payload: RecordingStateEvent }>>();

  return {
    getSettingsMock: vi.fn(),
    hidePillMock: vi.fn(),
    onPillTooltipMock: vi.fn(async (callback: Listener<PillTooltipEvent>) => {
      tooltipListeners.add(callback);
      return () => tooltipListeners.delete(callback);
    }),
    onSettingsChangedMock: vi.fn(async () => () => undefined),
    listenMock: vi.fn(async (eventName: string, callback: Listener<{ payload: RecordingStateEvent }>) => {
      if (eventName !== "recording-state-changed") {
        return () => undefined;
      }

      recordingStateListeners.add(callback);
      return () => recordingStateListeners.delete(callback);
    }),
    showPillMock: vi.fn(),
    emitPillTooltip: (event: PillTooltipEvent) => {
      tooltipListeners.forEach((callback) => callback(event));
    },
  };
});

const mocks = vi.hoisted(() => ({
  emitPillTooltip: undefined as
    | undefined
    | ((event: PillTooltipEvent) => void),
  emitRecordingState: undefined as
    | undefined
    | ((event: RecordingStateEvent) => void),
}));

vi.mock("@/lib/tauri", () => ({
  events: {
    onRecordingStateChanged: liveEvents.recording,
    onTranscriptionPartial: liveEvents.partial,
    onTranscriptionComplete: liveEvents.complete,
    onPillTooltip: onPillTooltipMock,
    onSettingsChanged: onSettingsChangedMock,
  },
  settingsCommands: {
    getSettings: getSettingsMock,
  },
  windowCommands: {
    hidePill: hidePillMock,
    showPill: showPillMock,
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    startDragging: vi.fn(),
  }),
}));

vi.mock("border-beam", () => ({
  BorderBeam: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock("../AudioDots", () => ({
  AudioDots: () => <div data-testid="audio-dots" />,
}));

vi.mock("../SettingsButton", () => ({
  SettingsButton: () => <button type="button" aria-label="settings" />,
}));

vi.mock("@/lib/logger", () => ({
  logger: {
    error: vi.fn(),
  },
}));

function settings(overrides: Partial<AppSettings> = {}): Partial<AppSettings> {
  return {
    pill_background_color: "#1d1d1d",
    pill_background_opacity: 1,
    pill_indicator_mode: "when_recording",
    pill_size: 2,
    ...overrides,
  };
}

describe("PillWindow backend tooltip", () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    await i18n.changeLanguage("en");
    getSettingsMock.mockReset();
    hidePillMock.mockReset();
    onPillTooltipMock.mockClear();
    onSettingsChangedMock.mockClear();
    showPillMock.mockReset();
    listenMock.mockClear();
    Object.values(liveEvents).forEach((subscribe) => subscribe.mockClear());
    hidePillMock.mockResolvedValue(undefined);
    showPillMock.mockResolvedValue(undefined);
    getSettingsMock.mockResolvedValue(settings());
    mocks.emitPillTooltip = (event: PillTooltipEvent) => {
      const calls = onPillTooltipMock.mock.calls as Array<[Listener<PillTooltipEvent>]>;
      calls.forEach(([callback]) => callback(event));
    };
    mocks.emitRecordingState = (event: RecordingStateEvent) => {
      liveEvents.recording.mock.calls.forEach(([callback]) => callback(event));
      const calls = listenMock.mock.calls as Array<[string, Listener<{ payload: RecordingStateEvent }>]>;
      calls
        .filter(([eventName]) => eventName === "recording-state-changed")
        .forEach(([, callback]) => callback({ payload: event }));
    };
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders compact live replacement text alongside existing tooltips", async () => {
    render(<PillWindow />);
    await act(async () => { await Promise.resolve(); });
    act(() => {
      mocks.emitRecordingState?.({ task_id: 10, status: "recording" });
      liveEvents.partial.mock.lastCall?.[0]({ task_id: 10, text: "Bonjour", is_definite: false });
    });
    expect(screen.getByText("Bonjour")).toHaveClass("overflow-hidden", "max-w-[calc(100vw-1rem)]");
    act(() => {
      liveEvents.partial.mock.lastCall?.[0]({ task_id: 10, text: "le monde", is_definite: false });
      mocks.emitPillTooltip?.({ task_id: 10, message: "Escape to cancel", duration_ms: 500 });
    });
    expect(screen.queryByText("Bonjour")).not.toBeInTheDocument();
    expect(screen.getByText("le monde")).toBeInTheDocument();
    expect(screen.getByText("Escape to cancel")).toBeInTheDocument();
    expect(showPillMock).not.toHaveBeenCalled();
    act(() => mocks.emitRecordingState?.({ task_id: 10, status: "polishing" }));
    expect(screen.queryByText("le monde")).not.toBeInTheDocument();
  });

  it("keeps the newest lines visible when live text exceeds the compact preview", async () => {
    render(<PillWindow />);
    await act(async () => { await Promise.resolve(); });
    act(() => {
      mocks.emitRecordingState?.({ task_id: 14, status: "recording" });
      liveEvents.partial.mock.lastCall?.[0]({ task_id: 14, text: "Initial words", is_definite: false });
    });
    const preview = screen.getByText("Initial words");
    Object.defineProperty(preview, "scrollHeight", { configurable: true, value: 160 });
    act(() => {
      liveEvents.partial.mock.lastCall?.[0]({
        task_id: 14,
        text: "Initial words followed by several lines of live transcription and the newest words",
        is_definite: false,
      });
    });
    expect(preview.scrollTop).toBe(160);
    expect(preview).not.toHaveClass("line-clamp-2");
  });

  it("does not show the indicator or live text in never mode", async () => {
    getSettingsMock.mockResolvedValue(settings({ pill_indicator_mode: "never", pill_size: 5 }));
    render(<PillWindow />);
    await act(async () => { await Promise.resolve(); });
    act(() => {
      mocks.emitRecordingState?.({ task_id: 11, status: "recording" });
      liveEvents.partial.mock.lastCall?.[0]({ task_id: 11, text: "Hidden live text", is_definite: false });
    });
    expect(screen.queryByText("Hidden live text")).not.toBeInTheDocument();
    expect(screen.queryByTestId("audio-dots")).not.toBeInTheDocument();
    expect(showPillMock).not.toHaveBeenCalled();
  });

  it("renders a live partial after mounting during recording without reopening the native window", async () => {
    render(<PillWindow />);
    await act(async () => { await Promise.resolve(); });
    act(() => liveEvents.partial.mock.lastCall?.[0]({ task_id: 12, text: "Already recording", is_definite: false }));
    expect(screen.getByText("Already recording")).toBeInTheDocument();
    expect(showPillMock).not.toHaveBeenCalled();
    act(() => mocks.emitRecordingState?.({ task_id: 12, status: "idle" }));
    act(() => liveEvents.partial.mock.lastCall?.[0]({ task_id: 12, text: "Late preview", is_definite: false }));
    expect(screen.getByText("Already recording")).not.toBeVisible();
    expect(screen.queryByText("Late preview")).not.toBeInTheDocument();
  });

  it("renders task-scoped tooltip messages without requesting a native window show", async () => {
    render(<PillWindow />);

    await act(async () => {
      await Promise.resolve();
    });
    expect(onPillTooltipMock).toHaveBeenCalled();

    act(() => {
      mocks.emitRecordingState?.({
        status: "recording",
        task_id: 2,
      });
    });

    act(() => {
      mocks.emitPillTooltip?.({
        message: "Escape to cancel, Enter to confirm",
        duration_ms: 3200,
        task_id: 2,
      });
    });

    expect(showPillMock).not.toHaveBeenCalled();
    expect(screen.getByTestId("audio-dots")).toBeInTheDocument();
    const tooltip = screen.getByText("Escape to cancel, Enter to confirm");
    expect(tooltip).toBeInTheDocument();
    expect(tooltip).toHaveClass("max-w-[calc(100vw-1rem)]");
  });

  it("renders fixed processing text for polish preview tooltips", async () => {
    render(<PillWindow />);

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      mocks.emitRecordingState?.({
        status: "polishing",
        task_id: 4,
      });
    });

    act(() => {
      mocks.emitPillTooltip?.({
        message: "Polishing preview:\nHello",
        duration_ms: 1600,
        task_id: 4,
      });
    });

    const tooltip = document.querySelector(".line-clamp-4");
    expect(tooltip).toBeInstanceOf(HTMLElement);
    expect(tooltip?.textContent).toBe(i18n.t("pill.polishPreviewProcessing"));
    expect(tooltip?.textContent).not.toContain("Hello");
    expect(tooltip).toHaveClass("line-clamp-4");
    expect(tooltip).toHaveClass("whitespace-normal");
    expect(tooltip).not.toHaveClass("truncate");

    act(() => {
      mocks.emitPillTooltip?.({
        message: "Polishing preview:\nHello world",
        duration_ms: 1600,
        task_id: 4,
      });
    });

    expect(tooltip?.textContent).toBe(i18n.t("pill.polishPreviewProcessing"));
    expect(tooltip?.textContent).not.toContain("Hello world");
  });

  it("renders idle backend tooltip messages without rendering the pill body", async () => {
    render(<PillWindow />);

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      mocks.emitPillTooltip?.({
        message: "Correction saved: search term -> sootie",
        duration_ms: 50,
        task_id: null,
      });
    });

    expect(showPillMock).toHaveBeenCalledTimes(1);
    const tooltip = screen.getByText("Correction saved: search term -> sootie");
    expect(tooltip).toBeInTheDocument();
    expect(screen.queryByTestId("audio-dots")).not.toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(50);
    });

    expect(tooltip).not.toBeVisible();
  });

  it("renders a fallback error tooltip when recording state enters error", async () => {
    render(<PillWindow />);

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      mocks.emitRecordingState?.({
        status: "error",
        task_id: 9,
      });
    });

    expect(showPillMock).not.toHaveBeenCalled();
    expect(screen.getByTestId("audio-dots")).toBeInTheDocument();
    expect(screen.getByText("Transcription failed. Please try again.")).toBeInTheDocument();
  });
});
