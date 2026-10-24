import { StrictMode, type ReactNode } from "react";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RecordingStateEvent, TranscriptionCompleteEvent, TranscriptionPartialEvent } from "@/lib/tauri";
import { useLiveTranscription } from "@/hooks/useLiveTranscription";

const subscriptions = vi.hoisted(() => ({
  recording: vi.fn<(callback: (event: RecordingStateEvent) => void) => Promise<() => void>>(),
  partial: vi.fn<(callback: (event: TranscriptionPartialEvent) => void) => Promise<() => void>>(),
  complete: vi.fn<(callback: (event: TranscriptionCompleteEvent) => void) => Promise<() => void>>(),
}));

vi.mock("@/lib/tauri", () => ({
  events: {
    onRecordingStateChanged: subscriptions.recording,
    onTranscriptionPartial: subscriptions.partial,
    onTranscriptionComplete: subscriptions.complete,
  },
}));
vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn() } }));

function recording(taskId: number, status = "recording") {
  act(() => subscriptions.recording.mock.lastCall?.[0]({ task_id: taskId, status }));
}

function partial(taskId: number, text: string) {
  act(() => subscriptions.partial.mock.lastCall?.[0]({ task_id: taskId, text, is_definite: false }));
}

function complete(taskId: number) {
  act(() => subscriptions.complete.mock.lastCall?.[0]({ task_id: taskId, text: "Final result" }));
}

beforeEach(() => {
  Object.values(subscriptions).forEach((subscribe) => {
    subscribe.mockReset();
    subscribe.mockImplementation(async () => vi.fn());
  });
});

describe("useLiveTranscription", () => {
  it("replaces overlapping recent-audio previews instead of appending", () => {
    const { result } = renderHook(useLiveTranscription);
    recording(10);
    partial(10, "Bonjour tout");
    expect(result.current).toBe("Bonjour tout");
    partial(10, "tout le monde");
    expect(result.current).toBe("tout le monde");
    partial(10, "");
    expect(result.current).toBe("");
  });

  it("ignores definite results and blank partials for an unseen task", () => {
    const { result } = renderHook(useLiveTranscription);
    act(() => subscriptions.partial.mock.lastCall?.[0]({ task_id: 10, text: "Final text", is_definite: true }));
    partial(10, "   ");
    expect(result.current).toBe("");
    partial(9, "Current preview");
    expect(result.current).toBe("Current preview");
    act(() => subscriptions.partial.mock.lastCall?.[0]({ task_id: 9, text: "Final text", is_definite: true }));
    expect(result.current).toBe("Current preview");
  });

  it("renders an authorized partial when mounted after recording started", () => {
    const { result } = renderHook(useLiveTranscription);
    partial(12, "Current session");
    expect(result.current).toBe("Current session");
    recording(12);
    expect(result.current).toBe("Current session");
    partial(11, "Old session");
    expect(result.current).toBe("Current session");
    recording(12, "idle");
    partial(12, "Late session");
    expect(result.current).toBe("");
  });

  it("accepts a newer authorized partial after a missed start but not an observed end", () => {
    const { result } = renderHook(useLiveTranscription);
    recording(13);
    partial(13, "Previous session");
    partial(14, "New session");
    expect(result.current).toBe("New session");
    recording(13, "idle");
    expect(result.current).toBe("New session");
    recording(14, "transcribing");
    partial(14, "Ended session");
    expect(result.current).toBe("");
  });

  it("clears on a newer session and ignores old state, partial and completion events", () => {
    const { result } = renderHook(useLiveTranscription);
    recording(20);
    partial(20, "Previous recording");
    recording(21);
    expect(result.current).toBe("");
    partial(21, "New recording");
    recording(20, "idle");
    partial(20, "Late preview");
    complete(20);
    expect(result.current).toBe("New recording");
    recording(21);
    expect(result.current).toBe("New recording");
  });

  it.each(["idle", "error", "transcribing", "processing", "polishing", "cancelled"])(
    "clears on %s and cannot be reopened by delayed same-task events",
    (status) => {
      const { result } = renderHook(useLiveTranscription);
      recording(30);
      partial(30, "Provisional text");
      recording(30, status);
      expect(result.current).toBe("");
      recording(30);
      partial(30, "Late provisional text");
      expect(result.current).toBe("");
      recording(31);
      partial(31, "Next recording");
      expect(result.current).toBe("Next recording");
    },
  );

  it("does not activate a recording after receiving its non-recording state first", () => {
    const { result } = renderHook(useLiveTranscription);
    recording(40, "transcribing");
    recording(40);
    partial(40, "Late preview");
    expect(result.current).toBe("");
  });

  it("clears on completion before idle and rejects late recording states", () => {
    const { result } = renderHook(useLiveTranscription);
    recording(50);
    partial(50, "Before completion");
    complete(50);
    expect(result.current).toBe("");
    recording(50);
    partial(50, "After completion");
    expect(result.current).toBe("");
    complete(51);
    recording(51);
    partial(51, "Already completed");
    expect(result.current).toBe("");
  });

  it("unsubscribes even when registration resolves after unmount", async () => {
    const pending: Array<() => void> = [];
    const cleanups = Object.values(subscriptions).map((subscribe) => {
      const cleanup = vi.fn();
      subscribe.mockImplementation(() => new Promise((resolve) => pending.push(() => resolve(cleanup))));
      return cleanup;
    });
    const { unmount } = renderHook(useLiveTranscription);
    unmount();
    await act(async () => pending.forEach((resolve) => resolve()));
    cleanups.forEach((cleanup) => expect(cleanup).toHaveBeenCalledTimes(1));
  });

  it("cleans up successful subscriptions independently when another registration rejects", async () => {
    subscriptions.partial.mockRejectedValue(new Error("Listener unavailable"));
    const { unmount } = renderHook(useLiveTranscription);
    await act(async () => { await Promise.resolve(); });
    const recordingCleanup = await subscriptions.recording.mock.results[0].value;
    const completeCleanup = await subscriptions.complete.mock.results[0].value;
    unmount();
    expect(recordingCleanup).toHaveBeenCalledTimes(1);
    expect(completeCleanup).toHaveBeenCalledTimes(1);
  });

  it("ignores callbacks from a disposed listener generation after remount", async () => {
    const wrapper = ({ children }: { children: ReactNode }) => <StrictMode>{children}</StrictMode>;
    const first = renderHook(useLiveTranscription, { wrapper });
    const disposedState = subscriptions.recording.mock.calls[0][0];
    const disposedPartial = subscriptions.partial.mock.calls[0][0];
    const disposedComplete = subscriptions.complete.mock.calls[0][0];
    first.unmount();
    const { result, unmount } = renderHook(useLiveTranscription, { wrapper });
    recording(60);
    partial(60, "Current generation");
    act(() => {
      disposedState({ task_id: 61, status: "recording" });
      disposedPartial({ task_id: 61, text: "Disposed generation", is_definite: false });
      disposedComplete({ task_id: 60, text: "Disposed final" });
    });
    expect(result.current).toBe("Current generation");
    unmount();
    await act(async () => { await Promise.resolve(); });
    for (const subscribe of Object.values(subscriptions)) {
      for (const registration of subscribe.mock.results) {
        const cleanup = await registration.value;
        expect(cleanup).toHaveBeenCalledTimes(1);
      }
    }
  });
});
