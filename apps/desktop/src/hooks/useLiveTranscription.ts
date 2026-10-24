import { useEffect, useRef, useState } from "react";
import { events } from "@/lib/tauri";
import { logger } from "@/lib/logger";

type ActiveTask = { id: number; recording: boolean };

export function useLiveTranscription(): string {
  const [text, setText] = useState("");
  const currentTask = useRef<ActiveTask | null>(null);

  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];

    const subscribe = (registration: Promise<() => void>) => {
      void registration.then((cleanup) => {
        if (disposed) cleanup();
        else cleanups.push(cleanup);
      }).catch((error: unknown) => {
        logger.error("live_transcription_listener_failed", { error: String(error) });
      });
    };

    subscribe(events.onRecordingStateChanged(({ task_id, status }) => {
      if (disposed || (currentTask.current && task_id < currentTask.current.id)) return;
      if (currentTask.current?.id === task_id) {
        if (status === "recording" || !currentTask.current.recording) return;
      }
      const recording = status === "recording";
      currentTask.current = { id: task_id, recording };
      setText("");
    }));

    subscribe(events.onTranscriptionPartial(({ task_id, text: preview, is_definite }) => {
      if (disposed || is_definite || (currentTask.current && task_id < currentTask.current.id)) return;
      if (currentTask.current?.id === task_id && !currentTask.current.recording) return;
      if (currentTask.current?.id !== task_id) {
        if (!preview.trim()) return;
        currentTask.current = { id: task_id, recording: true };
      }
      setText(preview);
    }));

    subscribe(events.onTranscriptionComplete(({ task_id }) => {
      if (disposed || (currentTask.current && task_id < currentTask.current.id)) return;
      currentTask.current = { id: task_id, recording: false };
      setText("");
    }));

    return () => {
      disposed = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, []);

  return text;
}
