import { beforeEach, expect, it, vi } from "vitest";
import { events, type TranscriptionPartialEvent } from "@/lib/tauri";

const { listenMock, unlisten } = vi.hoisted(() => ({ listenMock: vi.fn(), unlisten: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ Channel: vi.fn(), invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock, emit: vi.fn() }));

beforeEach(() => {
  vi.clearAllMocks();
  listenMock.mockResolvedValue(unlisten);
});

it("forwards the existing typed transcription-partial payload and cleanup unchanged", async () => {
  const callback = vi.fn();
  const cleanup = await events.onTranscriptionPartial(callback);
  expect(listenMock).toHaveBeenCalledWith("transcription-partial", expect.any(Function));
  const payload: TranscriptionPartialEvent = { task_id: 7, text: "Bonjour", is_definite: false };
  listenMock.mock.calls[0][1]({ payload });
  expect(callback).toHaveBeenCalledWith(payload);
  expect(cleanup).toBe(unlisten);
});
