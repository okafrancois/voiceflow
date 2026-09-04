import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { logger } from "./logger";

/** Wrapped invoke that logs command name, params (debug), timing (debug), and errors (error) */
function invokeWithLogging<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const start = performance.now();
  logger.debug(`ipc_request`, { command, args });

  return invoke<T>(command, args)
    .then((result) => {
      const duration_ms = Math.round(performance.now() - start);
      logger.debug(`ipc_response`, { command, duration_ms });
      return result;
    })
    .catch((error: unknown) => {
      const duration_ms = Math.round(performance.now() - start);
      logger.error(`ipc_error`, { command, duration_ms, error: String(error) });
      throw error;
    });
}

export interface Position {
  x: number;
  y: number;
}

export interface RecordingState {
  is_recording: boolean;
  is_transcribing: boolean;
  audio_level: number;
  output_path: string | null;
}

export interface RecordingStateEvent {
  status: string;
  task_id: number;
}

export interface TranscriptionCompleteEvent {
  text: string;
  task_id: number;
}

export interface CorrectionLearnedEvent {
  wrong: string;
  corrected: string;
  frequency: number;
}

export interface DictionaryEntry {
  term: string;
  aliases: string[];
  frequency: number;
  first_seen_at_ms: number;
  last_seen_at_ms: number;
  source: string;
}

export interface DictionaryImportResult {
  imported: number;
  skipped: number;
}

export interface PillTooltipEvent {
  message: string;
  duration_ms: number;
  task_id?: number | null;
}

export interface RetryStateEvent {
  entry_id: string;
  status: string;
  task_id: number;
}

export interface RetryCompleteEvent {
  entry_id: string;
  text: string;
  task_id: number;
}

export interface RetryErrorEvent {
  entry_id: string;
  error: string;
  task_id: number;
}

export interface AppUpdateInfo {
  version: string;
  currentVersion: string;
}

export type UpdateInstallEvent =
  | {
      event: "started";
      data: {
        contentLength?: number | null;
      };
    }
  | {
      event: "progress";
      data: {
        downloaded: number;
        contentLength?: number | null;
      };
    }
  | {
      event: "finished";
    };

export interface CloudProviderConfig {
  enabled: boolean;
  provider_type: string;
  api_key: string;
  base_url: string;
  model: string;
  enable_thinking: boolean;
}

export interface CloudSttConfig {
  enabled: boolean;
  provider_type: string;
  api_key: string;
  app_id: string;
  base_url: string;
  model: string;
  language: string;
}

export interface ProviderFieldSchema {
  name: string;
  key: string;
  required: boolean;
  default_value: string;
  example: string;
  secret: boolean;
}

export interface ProviderSchema {
  id: string;
  name: string;
  fields: ProviderFieldSchema[];
}

export interface CloudProviderSchemas {
  stt: ProviderSchema[];
  polish: ProviderSchema[];
}

export type CloudConnectionCheckKind =
  | "ok"
  | "disabled"
  | "missing_required"
  | "invalid_url"
  | "unsupported_provider"
  | "auth_failed"
  | "model_failed"
  | "network_failed"
  | "timeout"
  | "provider_error";

export interface CloudConnectionCheckResult {
  ok: boolean;
  kind: CloudConnectionCheckKind;
  message: string;
  duration_ms: number;
}

export type ShortcutTriggerMode = "hold" | "toggle" | "double_tap";

export interface ShortcutProfile {
  hotkey: string;
  trigger_mode: ShortcutTriggerMode;
  action: {
    Record?: {
      polish_template_id?: string | null;
    };
  };
}

export interface ShortcutProfilesMap {
  dictate: ShortcutProfile;
  riff: ShortcutProfile;
  custom?: ShortcutProfile;
}

export type WorkflowOutputAction = "insert" | "preview" | "copy";
export type OriginalTargetMode = "foreground" | "background";
export type ContextSource = "accessibility" | "clipboard" | "window_metadata" | "ocr";
export type VoiceActionKind = "shorten" | "translate" | "reply" | "list" | "custom";
export type QuickControlKind =
  | "undo_last_insertion"
  | "reinsert_raw"
  | "reinsert_final"
  | "copy_raw"
  | "copy_final"
  | "repolish"
  | "submit_enter"
  | "cancel_active_task";

export interface ContextCaptureSettings {
  application_metadata: boolean;
  focused_field: boolean;
  selected_text: boolean;
  clipboard: boolean;
  ocr_fallback: boolean;
}

export interface CapturedContext {
  application_id: string | null;
  application_name: string | null;
  window_title: string | null;
  focused_field_role: string | null;
  selected_text: string | null;
  clipboard_text: string | null;
  ocr_text: string | null;
  sources: ContextSource[];
  captured_at_ms: number;
}

export interface WorkflowProfile {
  id: string;
  name: string;
  hotkey: string;
  trigger_mode: ShortcutTriggerMode;
  language: string | null;
  polish_template_id: string | null;
  translation_target: string | null;
  output_action: WorkflowOutputAction;
  code_aware: boolean;
  protected: boolean;
}

export interface ApplicationRule {
  id: string;
  application_id: string;
  title_contains: string | null;
  profile_id: string;
  enabled: boolean;
}

export interface VoiceSnippet {
  id: string;
  spoken_trigger: string;
  template: string;
  enabled: boolean;
}

export interface WorkflowSettingsSnapshot {
  context_capture: ContextCaptureSettings;
  profiles: WorkflowProfile[];
  application_rules: ApplicationRule[];
  snippets: VoiceSnippet[];
}

export interface VoiceActionRequest {
  kind: VoiceActionKind;
  selected_text: string | null;
  translation_target: string | null;
  custom_instruction: string | null;
  output_action: WorkflowOutputAction;
}

export interface VoiceActionPreview {
  kind: VoiceActionKind;
  source_text: string;
  result_text: string;
  translation_target: string | null;
  output_action: WorkflowOutputAction;
}

export interface QuickControlResult {
  action: QuickControlKind;
  text: string | null;
}

export interface LocalPolishRuntimeSettings {
  provider_type: string;
  base_url: string;
  api_key: string;
  server_command: string;
  server_args_json: string;
  ready_timeout_secs: number;
}

export interface AppSettings {
  hotkey?: string;
  recording_mode: "hold" | "toggle";
  model: string;
  stt_engine: string;
  pill_position: string;
  pill_indicator_mode: string;
  auto_start: boolean;
  gpu_acceleration: boolean;
  language: string;
  stt_engine_language: string;
  beep_on_record: boolean;
  audio_device: string;
  polish_system_prompt: string;
  polish_model: string;
  theme_mode: "system" | "light" | "dark";
  stt_engine_initial_prompt: string;
  model_resident: boolean;
  idle_unload_minutes: number;
  denoise_mode: string;
  stt_engine_work_domain: string;
  stt_engine_work_domain_prompt: string;
  stt_engine_work_subdomain: string;
  stt_engine_user_glossary: string;
  custom_dictionary: string;
  analytics_opt_in: boolean;
  text_retention: RetentionPolicy;
  audio_retention: RetentionPolicy;
  cloud_stt_enabled: boolean;
  active_cloud_stt_provider: string;
  cloud_stt_configs: Record<string, CloudSttConfig>;
  cloud_polish_enabled: boolean;
  active_cloud_polish_provider: string;
  cloud_polish_configs: Record<string, CloudProviderConfig>;
  local_polish_runtime: LocalPolishRuntimeSettings;
  polish_stream_direct_typing_enabled: boolean;
  original_target_enabled: boolean;
  original_target_mode: OriginalTargetMode;
  vad_enabled: boolean;
  stay_in_tray: boolean;
  polish_custom_templates: CustomPolishTemplate[];
  shortcut_profiles: ShortcutProfilesMap;
  workflow_profiles: WorkflowProfile[];
  application_rules: ApplicationRule[];
  voice_snippets: VoiceSnippet[];
  context_capture: ContextCaptureSettings;
  window_context_enabled: boolean;
  pill_size: number;
  pill_background_color: string;
  pill_background_opacity: number;
  correction_memory_enabled: boolean;
}

export type RetentionPolicy = "never" | "days_7" | "days_30" | "days_90" | "forever";

export interface RetentionStatus {
  text_entries: number;
  audio_files: number;
  audio_bytes: number;
}

export interface ModelInfo {
  name: string;
  display_name: string;
  size_mb: number;
  url: string;
  downloaded: boolean;
  speed_score: number;
  accuracy_score: number;
}

export interface PolishModelInfo {
  id: string;
  name: string;
  size: string;
  downloaded: boolean;
  compatibility: PolishModelCompatibility;
  latency_profile: PolishModelLatencyProfile;
}

export interface PolishModelStatus {
  is_loaded: boolean;
  is_downloaded: boolean;
  runtime_ready: boolean;
  current_model: string;
  engine_type: string;
}

export type PolishModelLatencyClass = "fast" | "balanced" | "slow" | "heavy";

export interface PolishModelLatencyProfile {
  class: PolishModelLatencyClass;
  code:
    | "fast_transcript_preserving"
    | "balanced_rewrite"
    | "accurate_rewrite"
    | "heavy_long_context";
  recommended_templates: string[];
  caution_templates: string[];
}

export interface PolishModelCompatibility {
  level: "smooth" | "limited" | "unsupported";
  code:
    | "smooth"
    | "memory_unknown"
    | "memory_below_minimum"
    | "memory_below_recommended"
    | "cpu_threads_low";
  minimum_memory_mb: number;
  recommended_memory_mb: number;
  device_memory_mb: number | null;
  logical_cpu_count: number;
}

export interface RecommendedModel {
  engine_type: string;
  model_name: string;
  display_name: string;
  size_mb: number;
  speed_score: number;
  accuracy_score: number;
  downloaded: boolean;
}

export type SetupPreset = "private" | "balanced" | "maximum_accuracy";

export interface MicrophoneCheck {
  ready: boolean;
  device_name: string | null;
  sample_rate_hz: number | null;
  channels: number | null;
  peak_level: number | null;
  error: string | null;
}

export interface HardwareSnapshot {
  total_memory_mb: number | null;
  logical_cpu_count: number;
  architecture: string;
}

export interface ModelRecommendation {
  model_name: string;
  reason: string;
}

export interface LatencySample {
  stt_ms: number;
  polish_ms: number | null;
  total_ms: number;
  model_name: string;
}

export interface DiagnosticReport {
  microphone: MicrophoneCheck;
  hardware: HardwareSnapshot;
  recommended_model: ModelRecommendation;
  recommended_preset: SetupPreset;
  recommendation_reason: string;
  latency: LatencySample | null;
}

export interface CodeContext {
  language?: string | null;
  file_path?: string | null;
  symbol?: string | null;
  editor_id?: string | null;
}

export type QualityEventKind =
  | "transcription_success"
  | "transcription_failure"
  | "injection_failure"
  | "correction";

export interface QualityEvent {
  kind: QualityEventKind;
  application_id: string | null;
  stt_ms: number | null;
  polish_ms: number | null;
  total_ms: number | null;
  is_cloud: boolean | null;
  created_at_ms: number;
}

export interface QualityQuery {
  since_ms?: number | null;
  until_ms?: number | null;
  application_id?: string | null;
  kind?: QualityEventKind | null;
  is_cloud?: boolean | null;
}

export interface LatencyPercentiles {
  p50: number | null;
  p95: number | null;
}

export interface QualitySummary {
  total_transcriptions: number;
  transcription_failures: number;
  injection_failures: number;
  corrections: number;
  correction_rate_percent: number | null;
  local_transcriptions: number;
  cloud_transcriptions: number;
  stt_latency_ms: LatencyPercentiles;
  polish_latency_ms: LatencyPercentiles;
  total_latency_ms: LatencyPercentiles;
  application_injection_failures: Record<string, number>;
}

export const windowCommands = {
  showMain: () => invokeWithLogging("show_main_window"),
  hideMain: () => invokeWithLogging("hide_main_window"),
  showPill: () => invokeWithLogging("show_pill_window"),
  hidePill: () => invokeWithLogging("hide_pill_window"),
  updatePillPosition: (x: number, y: number) =>
    invokeWithLogging("update_pill_position", { x, y }),
  getPillPosition: () => invokeWithLogging<Position | null>("get_pill_position"),
};

export const audioCommands = {
  startRecording: () => invokeWithLogging<string>("start_recording"),
  stopRecording: () => invokeWithLogging<string | null>("stop_recording"),
  cancelRecording: () => invokeWithLogging<void>("cancel_recording"),
  getAudioLevel: () => invokeWithLogging<number>("get_audio_level"),
  getRecordingState: () => invokeWithLogging<RecordingState>("get_recording_state"),
};

export const textCommands = {
  insertText: (text: string) => invokeWithLogging("insert_text", { text }),
  copyToClipboard: (text: string) => invokeWithLogging("copy_to_clipboard", { text }),
  restoreClipboard: (text: string) => invokeWithLogging("restore_clipboard", { text }),
};

export const settingsCommands = {
  getSettings: () => invokeWithLogging<AppSettings>("get_settings"),
  updateSettings: (key: string, value: unknown) =>
    invokeWithLogging("update_settings", { key, value }),
  getGlossaryContent: (subdomain: string) =>
    invokeWithLogging<string>("get_glossary_content", { subdomain }),
  getAvailableSubdomains: (domain: string) =>
    invokeWithLogging<string[]>("get_available_subdomains", { domain }),
  getCloudProviderSchemas: () =>
    invokeWithLogging<CloudProviderSchemas>("get_cloud_provider_schemas"),
  checkActiveCloudSttConfig: () =>
    invokeWithLogging<CloudConnectionCheckResult>("check_active_cloud_stt_config"),
  checkActiveCloudPolishConfig: () =>
    invokeWithLogging<CloudConnectionCheckResult>("check_active_cloud_polish_config"),
  checkLocalPolishRuntimeConfig: () =>
    invokeWithLogging<CloudConnectionCheckResult>("check_local_polish_runtime_config"),
  clearCorrectionMemory: () =>
    invokeWithLogging<void>("clear_correction_memory"),
  openCorrectionMemoryDirectory: () =>
    invokeWithLogging<void>("open_correction_memory_directory"),
};

export const dictionaryCommands = {
  getAutoEntries: () =>
    invokeWithLogging<DictionaryEntry[]>("get_auto_dictionary_entries"),
  deleteAutoEntry: (term: string) =>
    invokeWithLogging<void>("delete_auto_dictionary_entry", { term }),
  getCustomEntries: () =>
    invokeWithLogging<DictionaryEntry[]>("get_custom_dictionary_entries"),
  addCustomEntry: (term: string) =>
    invokeWithLogging<DictionaryEntry>("add_custom_dictionary_entry", { term }),
  importCustomCsv: (csvContent: string) =>
    invokeWithLogging<DictionaryImportResult>("import_custom_dictionary_csv", { csvContent }),
  deleteCustomEntry: (term: string) =>
    invokeWithLogging<void>("delete_custom_dictionary_entry", { term }),
};

export const hotkeyCommands = {
  startCapture: (profileKey: string) => invokeWithLogging<void>("start_hotkey_capture", { profileKey }),
  stopCapture: (profileKey: string) => invokeWithLogging<string>("stop_hotkey_capture", { profileKey }),
  cancelCapture: () => invokeWithLogging<void>("cancel_hotkey_capture"),
  peekCapture: () => invokeWithLogging<string | null>("peek_hotkey_capture"),
  getProfiles: () => invokeWithLogging<ShortcutProfilesMap>("get_shortcut_profiles"),
  updateProfile: (key: string, profile: ShortcutProfile) =>
    invokeWithLogging<void>("update_shortcut_profile", { key, profile }),
  createCustom: (profile: ShortcutProfile) =>
    invokeWithLogging<void>("create_custom_profile", { profile }),
  deleteCustom: () =>
    invokeWithLogging<void>("delete_custom_profile"),
};

export const workflowCommands = {
  getSettings: () => invokeWithLogging<WorkflowSettingsSnapshot>("get_workflow_settings"),
  captureContext: () => invokeWithLogging<CapturedContext>("capture_workflow_context"),
  getLatestContext: () =>
    invokeWithLogging<CapturedContext | null>("get_latest_workflow_context"),
  resolveProfile: (requestedProfileId?: string | null) =>
    invokeWithLogging<WorkflowProfile>("resolve_workflow_profile", {
      requestedProfileId: requestedProfileId ?? null,
    }),
  createProfile: (profile: WorkflowProfile) =>
    invokeWithLogging<void>("create_workflow_profile", { profile }),
  updateProfile: (profile: WorkflowProfile) =>
    invokeWithLogging<void>("update_workflow_profile", { profile }),
  deleteProfile: (profileId: string) =>
    invokeWithLogging<void>("delete_workflow_profile", { profileId }),
  setApplicationRules: (rules: ApplicationRule[]) =>
    invokeWithLogging<void>("set_application_rules", { rules }),
  upsertApplicationRule: (rule: ApplicationRule) =>
    invokeWithLogging<void>("upsert_application_rule", { rule }),
  deleteApplicationRule: (ruleId: string) =>
    invokeWithLogging<void>("delete_application_rule", { ruleId }),
  setVoiceSnippets: (snippets: VoiceSnippet[]) =>
    invokeWithLogging<void>("set_voice_snippets", { snippets }),
  upsertVoiceSnippet: (snippet: VoiceSnippet) =>
    invokeWithLogging<void>("upsert_voice_snippet", { snippet }),
  deleteVoiceSnippet: (snippetId: string) =>
    invokeWithLogging<void>("delete_voice_snippet", { snippetId }),
  setContextCapture: (settingsValue: ContextCaptureSettings) =>
    invokeWithLogging<void>("set_context_capture_settings", { settingsValue }),
  expandVoiceSnippet: (spokenText: string) =>
    invokeWithLogging<string | null>("expand_voice_snippet", { spokenText }),
  runVoiceAction: (request: VoiceActionRequest) =>
    invokeWithLogging<VoiceActionPreview>("run_voice_action", { request }),
  replaceVoiceActionPreview: () =>
    invokeWithLogging<VoiceActionPreview>("replace_voice_action_preview"),
  recordDelivery: (
    rawText: string,
    finalText: string,
    insertedText: string,
    applicationId?: string | null,
  ) => invokeWithLogging<void>("record_workflow_delivery", {
    rawText,
    finalText,
    insertedText,
    applicationId: applicationId ?? null,
  }),
  runQuickControl: (action: QuickControlKind) =>
    invokeWithLogging<QuickControlResult>("run_quick_control", { action }),
};

export const systemCommands = {
  getAudioDevices: () => invokeWithLogging<string[]>("get_audio_devices"),
checkPermission: (kind: "accessibility" | "input_monitoring" | "microphone" | "screen_recording") =>
    invokeWithLogging<string | null>("check_permission", { kind }),

  applyPermission: (kind: "accessibility" | "input_monitoring" | "microphone" | "screen_recording") =>
    invokeWithLogging<void>("apply_permission", { kind }),
  getLogContent: (lines: number) => invokeWithLogging<string>("get_log_content", { lines }),
  openLogFolder: () => invokeWithLogging("open_log_folder"),
  getPlatform: () => invokeWithLogging<"macos" | "windows" | "linux" | "unknown">("get_platform"),
};

export const updateCommands = {
  check: () => invokeWithLogging<AppUpdateInfo | null>("check_for_update"),
  install: (onEvent: Channel<UpdateInstallEvent>) =>
    invokeWithLogging<void>("install_update", { onEvent }),
};

export const transcribeCommands = {
  transcribeAudio: (audioPath: string) =>
    invokeWithLogging<string>("transcribe_audio", { audioPath }),
  getSTTEngines: () => invokeWithLogging<string[]>("get_stt_engines"),
};

export const modelCommands = {
  getModels: () => invokeWithLogging<ModelInfo[]>("get_models"),
  isModelDownloaded: (modelName: string) =>
    invokeWithLogging<boolean>("is_model_downloaded", { modelName }),
  downloadModel: (modelName: string) =>
    invokeWithLogging<void>("download_model", { modelName }),
  cancelDownload: (modelName: string) =>
    invokeWithLogging<void>("cancel_download", { modelName }),
  deleteModel: (modelName: string) =>
    invokeWithLogging<void>("delete_model", { modelName }),
  recommendModelsByLanguage: (language: string) =>
    invokeWithLogging<RecommendedModel[]>("recommend_models_by_language", { language }),
  getPolishModels: () =>
    invokeWithLogging<PolishModelInfo[]>("get_polish_models"),
  getCurrentPolishModel: () =>
    invokeWithLogging<string>("get_current_polish_model"),
  getPolishModelStatus: () =>
    invokeWithLogging<PolishModelStatus>("get_polish_model_status"),
  preloadPolishModel: () =>
    invokeWithLogging<void>("preload_polish_model"),
  isPolishModelDownloaded: () =>
    invokeWithLogging<boolean>("is_polish_model_downloaded"),
  isPolishModelDownloadedForModel: (modelId: string) =>
    invokeWithLogging<boolean>("is_polish_model_downloaded_for_model", { modelId }),
  downloadPolishModel: () =>
    invokeWithLogging<void>("download_polish_model"),
  downloadPolishModelById: (modelId: string) =>
    invokeWithLogging<void>("download_polish_model_by_id", { modelId }),
  cancelPolishDownload: (modelId: string) =>
    invokeWithLogging<void>("cancel_polish_download", { modelId }),
  deletePolishModel: () =>
    invokeWithLogging<void>("delete_polish_model"),
  deletePolishModelById: (modelId: string) =>
    invokeWithLogging<void>("delete_polish_model_by_id", { modelId }),
  getPolishTemplates: () =>
    invokeWithLogging<PolishTemplate[]>("get_polish_templates"),
  getPolishTemplatePrompt: (templateId: string) =>
    invokeWithLogging<string>("get_polish_template_prompt", { templateId }),
  createPolishCustomTemplate: (name: string, systemPrompt: string) =>
    invokeWithLogging<CustomPolishTemplate>("create_polish_custom_template", { name, systemPrompt }),
  updatePolishCustomTemplate: (id: string, name: string, systemPrompt: string) =>
    invokeWithLogging<void>("update_polish_custom_template", { id, name, systemPrompt }),
  deletePolishCustomTemplate: (id: string) =>
    invokeWithLogging<void>("delete_polish_custom_template", { id }),
  getPolishCustomTemplates: () =>
    invokeWithLogging<CustomPolishTemplate[]>("get_polish_custom_templates"),
};

export interface PolishTemplate {
  id: string;
  name: string;
  description: string;
}

export interface CustomPolishTemplate {
  id: string;
  name: string;
  system_prompt: string;
}

export interface TranscriptionEntry {
  id: string;
  created_at: number;
  raw_text: string;
  final_text: string;
  stt_engine: string;
  stt_model: string | null;
  language: string | null;
  audio_duration_ms: number | null;
  stt_duration_ms: number | null;
  polish_duration_ms: number | null;
  total_duration_ms: number | null;
  polish_applied: boolean;
  polish_engine: string | null;
  is_cloud: boolean;
  /** Path to the saved audio file (for retry functionality). */
  audio_path: string | null;
  /** Status of the entry: "success" or "error". */
  status: string;
  /** Error message if transcription failed. */
  error: string | null;
  source_kind: "recording" | "file" | string;
  source_path: string | null;
  translation_target: string | null;
  timed_segments: TimedSegment[];
  delivery_status: string;
}

export interface TimedSegment {
  start_ms: number;
  end_ms: number;
  text: string;
}

export type ExportFormat = "txt" | "markdown" | "srt";
export type HistoryTextVersion = "raw" | "final";

export interface FileTranscriptionRequest {
  path: string;
  profile_id?: string | null;
  translation_target?: string | null;
}

export interface FileTranscriptionResult {
  history_entry_id: string | null;
  raw_text: string;
  final_text: string;
  source_path: string;
  translation_target: string | null;
  output_action: "preview" | "insert" | "copy" | string;
  delivery_status: string;
}

export type FileJobState = "queued" | "running" | "completed" | "error" | "canceled";

export interface FileTranscriptionJob {
  id: string;
  state: FileJobState;
  progress_percent: number;
  request: FileTranscriptionRequest;
  result: FileTranscriptionResult | null;
  error: string | null;
}

export interface HistoryAudioPayload {
  mime_type: string;
  bytes: number[];
}

export interface DashboardStats {
  total_count: number;
  today_count: number;
  total_chars: number;
  total_output_units: number;
  total_audio_ms: number;
  avg_stt_ms: number | null;
  avg_audio_ms: number | null;
  avg_output_units: number | null;
  local_count: number;
  cloud_count: number;
  polish_count: number;
  active_days: number;
  current_streak_days: number;
  longest_streak_days: number;
  last_7_days_count: number;
  last_7_days_audio_ms: number;
  last_7_days_output_units: number;
}

export interface DailyUsage {
  date: string;
  count: number;
  audio_ms: number;
  output_units: number;
}

export interface EngineUsage {
  engine: string;
  count: number;
  avg_stt_ms: number | null;
}

export interface HistoryFilter {
  search?: string;
  engine?: string;
  /** Filter by status: "success", "error", or undefined for all. */
  status?: string;
  date_from?: number;
  date_to?: number;
  limit?: number;
  offset?: number;
}

export const historyCommands = {
  getHistory: (filter: HistoryFilter) =>
    invokeWithLogging<TranscriptionEntry[]>("get_transcription_history", { filter }),
  getHistoryCount: (filter: HistoryFilter) =>
    invokeWithLogging<number>("get_history_count", { filter }),
  getEntry: (id: string) =>
    invokeWithLogging<TranscriptionEntry | null>("get_transcription_entry", { id }),
  getDashboardStats: () =>
    invokeWithLogging<DashboardStats>("get_dashboard_stats"),
  getDailyUsage: (days: number) =>
    invokeWithLogging<DailyUsage[]>("get_daily_usage", { days }),
  getEngineUsage: () =>
    invokeWithLogging<EngineUsage[]>("get_engine_usage"),
  getRetentionStatus: () =>
    invokeWithLogging<RetentionStatus>("get_retention_status"),
  deleteEntry: (id: string) =>
    invokeWithLogging<void>("delete_transcription_entry", { id }),
  clearAll: () =>
    invokeWithLogging<void>("clear_transcription_history"),
  retryTranscription: (id: string) =>
    invokeWithLogging<string>("retry_transcription", { id }),
  selectMediaFile: () =>
    invokeWithLogging<string | null>("select_media_file"),
  selectExportFile: (format: ExportFormat) =>
    invokeWithLogging<string | null>("select_export_file", { format }),
  transcribeMediaFile: (request: FileTranscriptionRequest) =>
    invokeWithLogging<FileTranscriptionResult>("transcribe_media_file", { request }),
  startFileJob: (request: FileTranscriptionRequest) =>
    invokeWithLogging<FileTranscriptionJob>("start_file_transcription_job", { request }),
  getFileJob: (id: string) =>
    invokeWithLogging<FileTranscriptionJob>("get_file_transcription_job", { id }),
  listFileJobs: () =>
    invokeWithLogging<FileTranscriptionJob[]>("list_file_transcription_jobs"),
  cancelFileJob: (id: string) =>
    invokeWithLogging<FileTranscriptionJob>("cancel_file_transcription_job", { id }),
  retranscribeEntry: (id: string) =>
    invokeWithLogging<TranscriptionEntry>("retranscribe_history_entry", { id }),
  repolishEntry: (
    id: string,
    templateId?: string | null,
    translationTarget?: string | null,
  ) =>
    invokeWithLogging<TranscriptionEntry>("repolish_history_entry", {
      id,
      templateId: templateId ?? null,
      translationTarget: translationTarget ?? null,
    }),
  exportEntry: (
    id: string,
    format: ExportFormat,
    outputPath: string,
    overwrite: boolean,
  ) =>
    invokeWithLogging<string>("export_history_entry", {
      id,
      format,
      outputPath,
      overwrite,
    }),
  getAudio: (id: string) =>
    invokeWithLogging<HistoryAudioPayload>("get_history_audio", { id }),
  copyEntry: (id: string, version: HistoryTextVersion) =>
    invokeWithLogging<void>("copy_history_entry", { id, version }),
  reinsertEntry: (id: string, version: HistoryTextVersion) =>
    invokeWithLogging<string>("reinsert_history_entry", { id, version }),
};

export const platformQualityCommands = {
  runDiagnostics: (microphoneSampleMs = 500) =>
    invokeWithLogging<DiagnosticReport>("run_setup_diagnostics", {
      microphoneSampleMs,
    }),
  runLatencyTest: (mediaPath: string) =>
    invokeWithLogging<LatencySample>("run_setup_latency_test", { mediaPath }),
  applyPreset: (preset: SetupPreset) =>
    invokeWithLogging<AppSettings>("apply_setup_preset", { preset }),
  setCodeContext: (context: CodeContext) =>
    invokeWithLogging<CodeContext>("set_code_context", { context }),
  getCodeContext: () =>
    invokeWithLogging<CodeContext | null>("get_code_context"),
  clearCodeContext: () => invokeWithLogging<void>("clear_code_context"),
  formatCodeTranscript: (text: string, language?: string | null) =>
    invokeWithLogging<string>("format_code_transcript", {
      text,
      language: language ?? null,
    }),
  getSummary: (query: QualityQuery) =>
    invokeWithLogging<QualitySummary>("get_quality_summary", { query }),
  getEvents: (query: QualityQuery) =>
    invokeWithLogging<QualityEvent[]>("get_quality_events", { query }),
  clearMetrics: () => invokeWithLogging<number>("clear_quality_metrics"),
  exportMetrics: (path: string, query: QualityQuery, overwrite = false) =>
    invokeWithLogging<string>("export_quality_metrics", { path, query, overwrite }),
};

export const events = {
  onFileTranscriptionJobChanged: (callback: (job: FileTranscriptionJob) => void) =>
    listen<FileTranscriptionJob>("file-transcription-job-changed", (event) => callback(event.payload)),
  onRecordingStateChanged: (callback: (event: RecordingStateEvent) => void) => {
    return listen<RecordingStateEvent>("recording-state-changed", (event) => {
      const { task_id, status } = event.payload;
      logger.info("event_received-recording_state_changed", { task_id, status });
      callback(event.payload);
    });
  },
  onAudioLevel: (callback: (level: number) => void) => {
    return listen<number>("audio-level", (event) => {
      const level = event.payload;
      logger.debug("event_received-audio_level", { level });
      callback(event.payload);
    });
  },
  onTranscriptionComplete: (callback: (event: TranscriptionCompleteEvent) => void) => {
    return listen<TranscriptionCompleteEvent>("transcription-complete", (event) => {
      const { task_id, text } = event.payload;
      logger.info("event_received-transcription_complete", { task_id, text_len: text.length });
      callback(event.payload);
    });
  },
  onCorrectionLearned: (callback: (event: CorrectionLearnedEvent) => void) => {
    return listen<CorrectionLearnedEvent>("correction-learned", (event) => {
      const { frequency, wrong, corrected } = event.payload;
      logger.info("event_received-correction_learned", {
        frequency,
        wrong_len: wrong.length,
        corrected_len: corrected.length,
      });
      callback(event.payload);
    });
  },
  onPillTooltip: (callback: (event: PillTooltipEvent) => void) => {
    return listen<PillTooltipEvent>("pill-tooltip", (event) => {
      const { message, duration_ms, task_id } = event.payload;
      logger.info("event_received-pill_tooltip", {
        message_len: message.length,
        duration_ms,
        task_id,
      });
      callback(event.payload);
    });
  },
  onRetryStateChanged: (callback: (event: RetryStateEvent) => void) => {
    return listen<RetryStateEvent>("retry-state-changed", (event) => {
      const { entry_id, task_id, status } = event.payload;
      logger.info("event_received-retry_state_changed", { entry_id, task_id, status });
      callback(event.payload);
    });
  },
  onRetryComplete: (callback: (event: RetryCompleteEvent) => void) => {
    return listen<RetryCompleteEvent>("retry-complete", (event) => {
      const { entry_id, task_id, text } = event.payload;
      logger.info("event_received-retry_complete", { entry_id, task_id, text_len: text.length });
      callback(event.payload);
    });
  },
  onRetryError: (callback: (event: RetryErrorEvent) => void) => {
    return listen<RetryErrorEvent>("retry-error", (event) => {
      const { entry_id, task_id, error } = event.payload;
      logger.error("event_received-retry_error", { entry_id, task_id, error });
      callback(event.payload);
    });
  },
  onTranscriptionError: (callback: (error: string) => void) => {
    return listen<string>("transcription-error", (event) => {
      const error = event.payload;
      logger.error("event_received-transcription_error", { error });
      callback(event.payload);
    });
  },
  onModelDownloadProgress: (
    callback: (data: {
      model: string;
      downloaded: number;
      total: number;
      progress: number;
    }) => void
  ) => {
    return listen<{
      model: string;
      downloaded: number;
      total: number;
      progress: number;
    }>("model-download-progress", (event) => {
      const { model, downloaded, total, progress } = event.payload;
      logger.debug("event_received-model_download_progress", { model, downloaded, total, progress });
      callback(event.payload);
    });
  },
  onModelDownloadComplete: (callback: (model: string) => void) => {
    return listen<{ model: string }>("model-download-complete", (event) => {
      const model = event.payload.model;
      logger.info("event_received-model_download_complete", { model });
      callback(event.payload.model);
    });
  },
  onModelDownloadCancelled: (callback: (model: string) => void) => {
    return listen<{ model: string }>("model-download-cancelled", (event) => {
      const model = event.payload.model;
      logger.info("event_received-model_download_cancelled", { model });
      callback(event.payload.model);
    });
  },
  onModelDeleted: (callback: (model: string) => void) => {
    return listen<{ model: string }>("model-deleted", (event) => {
      const model = event.payload.model;
      logger.info("event_received-model_deleted", { model });
      callback(event.payload.model);
    });
  },
  onPolishModelDownloadProgress: (
    callback: (data: { model_id: string; downloaded: number; total: number; progress: number }) => void
  ) => {
    return listen<{ model_id: string; downloaded: number; total: number; progress: number }>(
      "polish-model-download-progress",
      (event) => {
        const { model_id, downloaded, total, progress } = event.payload;
        logger.debug("event_received-polish_model_download_progress", { model_id, downloaded, total, progress });
        callback(event.payload);
      }
    );
  },
  onPolishModelDownloadComplete: (callback: (model_id: string) => void) => {
    return listen<{ model_id: string }>("polish-model-download-complete", (event) => {
      const model_id = event.payload.model_id;
      logger.info("event_received-polish_model_download_complete", { model_id });
      callback(event.payload.model_id);
    });
  },
  onPolishModelDownloadCancelled: (callback: (model_id: string) => void) => {
    return listen<{ model_id: string }>("polish-model-download-cancelled", (event) => {
      const model_id = event.payload.model_id;
      logger.info("event_received-polish_model_download_cancelled", { model_id });
      callback(event.payload.model_id);
    });
  },
  onPolishModelDeleted: (callback: () => void) => {
    return listen("polish-model-deleted", () => {
      logger.info("event_received-polish_model_deleted");
      callback();
    });
  },
  onToastMessage: (callback: (message: string) => void) => {
    return listen<string>("toast-message", (event) => {
      logger.debug("event_received-toast_message", { message: event.payload });
      callback(event.payload);
    });
  },
  onShortcutRegistrationFailed: (callback: (payload: { error: string; profile_id: string }) => void) => {
    return listen<{ error: string; profile_id: string }>("shortcut-registration-failed", (event) => {
      const { error, profile_id } = event.payload;
      logger.error("shortcut_registration_failed", { error, profile_id });
      callback(event.payload);
    });
  },
  onShortcutTriggered: (callback: (payload: { state: string; profile_id: string }) => void) => {
    return listen<{ state: string; profile_id: string }>("shortcut-triggered", (event) => {
      const { state, profile_id } = event.payload;
      logger.info("event_received-shortcut_triggered", { state, profile_id });
      callback(event.payload);
    });
  },
  onHotkeyCaptured: (callback: (hotkey: string) => void) => {
    return listen<string>("hotkey-captured", (event) => {
      const hotkey = event.payload;
      logger.info("event_received-hotkey_captured", { hotkey });
      callback(event.payload);
    });
  },
  onSettingsChanged: (callback: (settings: AppSettings) => void) => {
    return listen<AppSettings>("settings-changed", (event) => {
      logger.info("event_received-settings_changed");
      callback(event.payload);
    });
  },
  emit: (event: string, payload?: unknown) => emit(event, payload),
};
