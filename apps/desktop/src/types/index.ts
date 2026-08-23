export type RecordingStatus = "idle" | "recording" | "transcribing" | "processing" | "polishing" | "error";

export interface PillPosition {
  x: number;
  y: number;
}

export type PillIndicatorMode = "always" | "when_recording" | "never";

export type PresetPosition =
  | "top-left"
  | "top-center"
  | "top-right"
  | "bottom-left"
  | "bottom-center"
  | "bottom-right";

export type LocalSttModel =
  | "sense-voice-small"
  | "whisper-tiny"
  | "whisper-base"
  | "whisper-small"
  | "whisper-medium"
  | "whisper-large-v3"
  | "whisper-turbo"
  | "qwen3-asr-0.6b-int8";
