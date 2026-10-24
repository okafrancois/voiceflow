import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ModelSettings } from "../ModelSettings";
import type { AppSettings, ModelInfo, PolishModelInfo, PolishModelStatus } from "@/lib/tauri";

const {
  downloadPolishModelByIdMock,
  downloadModelMock,
  cancelDownloadMock,
  getModelsMock,
  getPolishModelsMock,
  getPolishModelStatusMock,
  onModelDeletedMock,
  onModelDownloadCancelledMock,
  onModelDownloadCompleteMock,
  onModelDownloadProgressMock,
  onPolishModelDeletedMock,
  onPolishModelDownloadCancelledMock,
  onPolishModelDownloadCompleteMock,
  onPolishModelDownloadProgressMock,
  preloadPolishModelMock,
  updateSettingContextMock,
  updateSettingsCommandMock,
  showErrorToastMock,
} = vi.hoisted(() => ({
  downloadPolishModelByIdMock: vi.fn(),
  downloadModelMock: vi.fn(),
  cancelDownloadMock: vi.fn(),
  getModelsMock: vi.fn(),
  getPolishModelsMock: vi.fn(),
  getPolishModelStatusMock: vi.fn(),
  onModelDeletedMock: vi.fn(),
  onModelDownloadCancelledMock: vi.fn(),
  onModelDownloadCompleteMock: vi.fn(),
  onModelDownloadProgressMock: vi.fn(),
  onPolishModelDeletedMock: vi.fn(),
  onPolishModelDownloadCancelledMock: vi.fn(),
  onPolishModelDownloadCompleteMock: vi.fn(),
  onPolishModelDownloadProgressMock: vi.fn(),
  preloadPolishModelMock: vi.fn(),
  updateSettingContextMock: vi.fn(),
  updateSettingsCommandMock: vi.fn(),
  showErrorToastMock: vi.fn(),
}));

vi.mock("@/contexts/SettingsContext", () => ({
  useSettingsContext: () => ({
    settings: testSettings,
    loading: false,
    polishAvailable: false,
    updateSetting: updateSettingContextMock,
  }),
}));

vi.mock("@/lib/tauri", () => ({
  modelCommands: {
    downloadModel: downloadModelMock,
    cancelDownload: cancelDownloadMock,
    downloadPolishModelById: downloadPolishModelByIdMock,
    getModels: getModelsMock,
    getPolishModels: getPolishModelsMock,
    getPolishModelStatus: getPolishModelStatusMock,
    preloadPolishModel: preloadPolishModelMock,
  },
  settingsCommands: {
    checkLocalPolishRuntimeConfig: vi.fn(async () => ({
      ok: true,
      kind: "ok",
      message: "ok",
      duration_ms: 1,
    })),
    updateSettings: updateSettingsCommandMock,
  },
  events: {
    onModelDeleted: onModelDeletedMock,
    onModelDownloadCancelled: onModelDownloadCancelledMock,
    onModelDownloadComplete: onModelDownloadCompleteMock,
    onModelDownloadProgress: onModelDownloadProgressMock,
    onPolishModelDeleted: onPolishModelDeletedMock,
    onPolishModelDownloadCancelled: onPolishModelDownloadCancelledMock,
    onPolishModelDownloadComplete: onPolishModelDownloadCompleteMock,
    onPolishModelDownloadProgress: onPolishModelDownloadProgressMock,
  },
}));

vi.mock("@/lib/toast", () => ({
  showErrorToast: showErrorToastMock,
}));

vi.mock("@/lib/analytics", () => ({
  analytics: {
    track: vi.fn(),
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: {
    error: vi.fn(),
  },
}));

vi.mock("react-i18next", () => ({
  initReactI18next: {
    init: vi.fn(),
    type: "3rdParty",
  },
  useTranslation: () => ({
    i18n: {
      changeLanguage: vi.fn(async () => undefined),
    },
    t: (key: string, values?: Record<string, unknown>) => {
      if (values?.templates) return `${key}:${values.templates}`;
      return key;
    },
  }),
}));

const testSettings: AppSettings = {
  active_cloud_polish_provider: "openai",
  active_cloud_stt_provider: "volcengine",
  analytics_opt_in: false,
  developer_bridge_enabled: false,
  vibe_coding_enabled: false,
  audio_retention: "never",
  audio_device: "default",
  auto_start: false,
  beep_on_record: true,
  cloud_polish_configs: {},
  cloud_polish_enabled: false,
  cloud_stt_configs: {},
  cloud_stt_enabled: false,
  correction_memory_enabled: true,
  denoise_mode: "off",
  gpu_acceleration: false,
  hotkey: "Cmd+Slash",
  idle_unload_minutes: 5,
  language: "auto",
  local_polish_runtime: {
    provider_type: "llama-server",
    base_url: "http://127.0.0.1:8000/v1",
    api_key: "",
    server_command: "",
    server_args_json: "",
    ready_timeout_secs: 20,
  },
  model: "tiny",
  model_resident: false,
  pill_background_color: "#1d1d1d",
  pill_background_opacity: 1,
  pill_indicator_mode: "always",
  pill_position: "bottom-right",
  pill_size: 2,
  polish_custom_templates: [],
  polish_model: "qwen3.5-2b",
  original_target_enabled: false,
  original_target_mode: "foreground",
  polish_system_prompt: "",
  recording_mode: "hold",

  workflow_profiles: [],
  application_rules: [],
  voice_snippets: [],
  context_capture: {
    application_metadata: true,
    focused_field: true,
    selected_text: true,
    clipboard: false,
    ocr_fallback: false,
  },
  stay_in_tray: false,
  stt_engine: "sherpa",
  stt_engine_initial_prompt: "",
  stt_engine_language: "en",
  stt_engine_user_glossary: "",
  custom_dictionary: "",
  stt_engine_work_domain: "general",
  stt_engine_work_domain_prompt: "",
  stt_engine_work_subdomain: "general",
  theme_mode: "system",
  text_retention: "days_90",
  vad_enabled: false,
  window_context_enabled: false,
};

const polishModel: PolishModelInfo = {
  id: "qwen3.5-0.8b",
  name: "Qwen 3.5 0.8B",
  size: "600MB",
  downloaded: false,
  compatibility: {
    level: "smooth",
    code: "smooth",
    minimum_memory_mb: 2048,
    recommended_memory_mb: 4096,
    device_memory_mb: 16384,
    logical_cpu_count: 10,
  },
  latency_profile: {
    class: "fast",
    code: "fast_transcript_preserving",
    recommended_templates: ["filler"],
    caution_templates: [],
  },
  built_in: false,
};

const voiceModel: ModelInfo = {
  name: "whisper-turbo",
  display_name: "Whisper Turbo INT8 (989M)",
  size_mb: 989,
  url: "",
  downloaded: false,
  speed_score: 5,
  accuracy_score: 10,
  built_in: false,
};

const appleModel: ModelInfo = {
  name: "apple-speech",
  display_name: "Apple (fast)",
  size_mb: 0,
  url: "",
  downloaded: true,
  speed_score: 10,
  accuracy_score: 7,
  built_in: true,
};

const mlxModels = [
  { name: "mlx-whisper-turbo", display_name: "MLX Whisper Turbo", language_codes: [] },
  { name: "mlx-parakeet-v3", display_name: "MLX Parakeet v3", language_codes: ["en", "fr", "de"] },
  { name: "mlx-qwen3-asr-0.6b", display_name: "MLX Qwen3-ASR 0.6B", language_codes: ["en", "zh", "fr"] },
].map(model => ({
  ...voiceModel,
  ...model,
  engine: "mlx",
  runtime_ready: false,
  speed_score: 0,
  accuracy_score: 0,
}));

const polishStatus: PolishModelStatus = {
  is_loaded: false,
  is_downloaded: false,
  runtime_ready: false,
  current_model: "qwen3.5-2b",
  engine_type: "qwen",
};

function registerEventMocks() {
  const cleanup = vi.fn();
  onModelDeletedMock.mockImplementation(async () => cleanup);
  onModelDownloadCancelledMock.mockImplementation(async () => cleanup);
  onModelDownloadCompleteMock.mockImplementation(async () => cleanup);
  onModelDownloadProgressMock.mockImplementation(async () => cleanup);
  onPolishModelDeletedMock.mockImplementation(async () => cleanup);
  onPolishModelDownloadCancelledMock.mockImplementation(async () => cleanup);
  onPolishModelDownloadCompleteMock.mockImplementation(async () => cleanup);
  onPolishModelDownloadProgressMock.mockImplementation(async () => cleanup);
}

function pendingModelResponse() {
  let complete: (models: ModelInfo[]) => void = () => { throw new Error("Model response is not initialized"); };
  const promise = new Promise<ModelInfo[]>((resolve) => { complete = resolve; });
  return { promise, complete };
}

describe("ModelSettings polish model activation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    registerEventMocks();
    getModelsMock.mockResolvedValue([]);
    getPolishModelsMock.mockResolvedValue([polishModel]);
    getPolishModelStatusMock.mockResolvedValue(polishStatus);
    downloadPolishModelByIdMock.mockResolvedValue(undefined);
    preloadPolishModelMock.mockResolvedValue(undefined);
    updateSettingContextMock.mockResolvedValue(undefined);
    downloadModelMock.mockResolvedValue(undefined);
    cancelDownloadMock.mockResolvedValue(undefined);
    updateSettingsCommandMock.mockResolvedValue(undefined);
  });

  it("selects and preloads a polish model after its download completes", async () => {
    render(<ModelSettings />);

    fireEvent.click(screen.getByRole("button", { name: "model.tabs.polish" }));
    fireEvent.click(await screen.findByRole("button", { name: "model.available.download" }));

    await waitFor(() => {
      expect(downloadPolishModelByIdMock).toHaveBeenCalledWith("qwen3.5-0.8b");
    });
    await waitFor(() => {
      expect(onPolishModelDownloadCompleteMock).toHaveBeenCalledTimes(1);
    });

    const callback = onPolishModelDownloadCompleteMock.mock.calls[0]?.[0];
    expect(callback).toBeTypeOf("function");
    await act(async () => {
      callback("qwen3.5-0.8b");
    });

    await waitFor(() => {
      expect(updateSettingsCommandMock).toHaveBeenCalledWith("polish_model", "qwen3.5-0.8b");
      expect(preloadPolishModelMock).toHaveBeenCalledTimes(1);
    });
  });
});

describe("ModelSettings voice model downloads", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    registerEventMocks();
    getModelsMock.mockResolvedValue([voiceModel]);
    getPolishModelsMock.mockResolvedValue([]);
    getPolishModelStatusMock.mockResolvedValue(polishStatus);
    updateSettingContextMock.mockResolvedValue(undefined);
  });

  it("offers backend-authorized Apple asset installation without changing model selection", async () => {
    getModelsMock.mockResolvedValue([{ ...appleModel, downloaded: false, downloadable: true }]);
    downloadModelMock.mockResolvedValue(undefined);
    render(<ModelSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "model.available.download" }));
    await waitFor(() => expect(downloadModelMock).toHaveBeenCalledWith("apple-speech"));
    expect(updateSettingContextMock).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "model.available.delete" })).toBeNull();
  });

  it("ignores older model readiness responses after a language change", async () => {
    const previousLanguage = testSettings.stt_engine_language;
    const first = pendingModelResponse();
    const latest = pendingModelResponse();
    getModelsMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(latest.promise);
    const view = render(<ModelSettings />);
    try {
      await waitFor(() => expect(getModelsMock).toHaveBeenCalledTimes(1));
      testSettings.stt_engine_language = previousLanguage === "fr" ? "en" : "fr";
      view.rerender(<ModelSettings />);
      await waitFor(() => expect(getModelsMock).toHaveBeenCalledTimes(2));
      await act(async () => latest.complete([{ ...appleModel, downloaded: false }]));
      expect(await screen.findByText("model.catalog.unavailable")).toBeTruthy();
      await act(async () => first.complete([appleModel]));
      expect(screen.queryByRole("button", { name: "model.catalog.use" })).toBeNull();
      expect(screen.getByText("model.catalog.unavailable")).toBeTruthy();
    } finally {
      first.complete([]);
      latest.complete([]);
      testSettings.stt_engine_language = previousLanguage;
      getModelsMock.mockReset().mockResolvedValue([voiceModel]);
    }
  });

  it("refreshes backend model readiness after the dictation language changes", async () => {
    const previousLanguage = testSettings.stt_engine_language;
    getModelsMock.mockResolvedValueOnce([appleModel]).mockResolvedValueOnce([{ ...appleModel, downloaded: false }]);
    const view = render(<ModelSettings />);
    try {
      await waitFor(() => expect(getModelsMock).toHaveBeenCalledTimes(1));
      testSettings.stt_engine_language = previousLanguage === "fr" ? "en" : "fr";
      view.rerender(<ModelSettings />);
      await waitFor(() => expect(getModelsMock).toHaveBeenCalledTimes(2));
      expect(await screen.findByText("model.catalog.unavailable")).toBeTruthy();
      expect(screen.queryByRole("button", { name: "model.catalog.use" })).toBeNull();
    } finally {
      testSettings.stt_engine_language = previousLanguage;
      getModelsMock.mockReset().mockResolvedValue([voiceModel]);
    }
  });

  it("shows the backend error when a model download fails", async () => {
    downloadModelMock.mockRejectedValueOnce(new Error("download integrity check failed"));

    render(<ModelSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "model.available.download" }));

    await waitFor(() => {
      expect(showErrorToastMock).toHaveBeenCalledWith("download integrity check failed");
    });
  });

  it("shows a built-in model without size or delete action", async () => {
    getModelsMock.mockResolvedValue([appleModel]);

    render(<ModelSettings />);

    expect(await screen.findAllByText(/model\.available\.builtIn/)).not.toHaveLength(0);
    expect(screen.queryByText(/0MB/)).toBeNull();
    expect(screen.queryByRole("button", { name: "model.available.delete" })).toBeNull();
  });

  it("combines disk size and availability filters without changing selection", async () => {
    getModelsMock.mockResolvedValue([
      voiceModel,
      { ...voiceModel, name: "whisper-base", display_name: "Whisper Base", size_mb: 279, downloaded: true },
      { ...voiceModel, name: "whisper-tiny", display_name: "Whisper Tiny", size_mb: 99 },
    ]);
    render(<ModelSettings />);
    const catalogue = await screen.findByRole("region", { name: "model.catalog.title" });
    fireEvent.change(within(catalogue).getByRole("combobox", { name: "model.catalog.size" }), { target: { value: "500" } });
    expect(within(catalogue).queryByRole("article", { name: voiceModel.display_name })).toBeNull();
    fireEvent.click(within(catalogue).getByRole("checkbox", { name: "model.catalog.readyOnly" }));
    expect(within(catalogue).getAllByRole("article")).toHaveLength(1);
    expect(within(catalogue).getByRole("article", { name: "Whisper Base" })).toBeInTheDocument();
    expect(updateSettingContextMock).not.toHaveBeenCalled();
  });

  it("filters restricted languages using backend metadata and restores all models", async () => {
    getModelsMock.mockResolvedValue([
      voiceModel,
      { ...voiceModel, name: "sense-voice-small", display_name: "SenseVoice Small", language_codes: ["zh", "ja", "ko", "en"] },
      { ...voiceModel, name: "restricted-example", display_name: "French Model", language_codes: ["fr"] },
    ]);
    render(<ModelSettings />);
    const catalogue = await screen.findByRole("region", { name: "model.catalog.title" });
    fireEvent.change(within(catalogue).getByRole("combobox", { name: "model.catalog.language" }), { target: { value: "current" } });
    expect(within(catalogue).queryByRole("article", { name: "French Model" })).toBeNull();
    expect(within(catalogue).getByRole("article", { name: "SenseVoice Small" })).toBeInTheDocument();
    fireEvent.change(within(catalogue).getByRole("combobox", { name: "model.catalog.language" }), { target: { value: "all" } });
    expect(within(catalogue).getByRole("article", { name: "French Model" })).toBeInTheDocument();
  });

  it("shows the active MLX service and forwards only an explicit model selection", async () => {
    const previous = { enabled: testSettings.cloud_stt_enabled, provider: testSettings.active_cloud_stt_provider };
    testSettings.cloud_stt_enabled = true;
    testSettings.active_cloud_stt_provider = "mlx-audio";
    getModelsMock.mockResolvedValue([{ ...voiceModel, downloaded: true }]);
    try {
      render(<ModelSettings />);
      await screen.findByRole("article", { name: voiceModel.display_name });
      expect(screen.getByText(/MLX Audio/)).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "model.catalog.selected" })).toBeNull();
      expect(updateSettingContextMock).not.toHaveBeenCalled();
      await act(async () => fireEvent.click(screen.getByRole("button", { name: "model.catalog.use" })));
      await waitFor(() => {
        expect(updateSettingContextMock).toHaveBeenNthCalledWith(1, "model", voiceModel.name);
        expect(updateSettingContextMock).toHaveBeenCalledTimes(1);
      });
    } finally {
      testSettings.cloud_stt_enabled = previous.enabled;
      testSettings.active_cloud_stt_provider = previous.provider;
    }
  });
});

describe("ModelSettings managed MLX models", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    registerEventMocks();
    getModelsMock.mockResolvedValue(mlxModels);
    getPolishModelsMock.mockResolvedValue([]);
    getPolishModelStatusMock.mockResolvedValue(polishStatus);
    downloadModelMock.mockResolvedValue(undefined);
    cancelDownloadMock.mockResolvedValue(undefined);
    updateSettingContextMock.mockResolvedValue(undefined);
  });

  it("renders MLX choices without infrastructure copy, empty subtitles, or invented rankings", async () => {
    render(<ModelSettings />);

    const parakeet = await screen.findByRole("article", { name: "MLX Parakeet v3" });
    expect(screen.getAllByRole("article")).toHaveLength(3);
    expect(screen.queryByText("model.mlx.managedDescription")).toBeNull();
    expect(screen.queryByText("model.mlx.modelDescription")).toBeNull();
    expect(screen.queryByText("model.catalog.sttNote")).toBeNull();
    for (const row of screen.getAllByRole("article")) {
      expect(row.querySelector(".model-catalog-description")).toBeNull();
      expect(within(row).queryByRole("img")).toBeNull();
      expect(within(row).queryByText("0/10")).toBeNull();
      expect(within(row).getAllByText("model.catalog.unmeasured")).toHaveLength(2);
    }
    expect(within(parakeet).queryByText("model.hint.cjkBest")).toBeNull();
    fireEvent.click(within(parakeet).getByRole("button", { name: "model.available.download" }));
    await waitFor(() => expect(downloadModelMock).toHaveBeenCalledWith("mlx-parakeet-v3"));
  });

  it("keeps useful native descriptions and catalogue ratings unchanged", async () => {
    getModelsMock.mockResolvedValue([voiceModel, appleModel]);
    render(<ModelSettings />);

    const native = await screen.findByRole("article", { name: voiceModel.display_name });
    expect(within(native).getByRole("img", { name: "model.available.speed: 5/10" })).toBeInTheDocument();
    expect(within(native).getByRole("img", { name: "model.available.accuracy: 10/10" })).toBeInTheDocument();
    expect(within(native).getByText("model.hint.multiLang")).toBeInTheDocument();
    expect(screen.getByText("model.catalog.sttNote")).toBeInTheDocument();
    expect(screen.queryByText("model.mlx.managedDescription")).toBeNull();
  });

  it("preserves numeric download progress for native models without a stage field", async () => {
    getModelsMock.mockResolvedValue([voiceModel]);
    render(<ModelSettings />);
    const row = await screen.findByRole("article", { name: voiceModel.display_name });
    fireEvent.click(within(row).getByRole("button", { name: "model.available.download" }));
    await waitFor(() => expect(onModelDownloadProgressMock).toHaveBeenCalledTimes(1));

    await act(async () => onModelDownloadProgressMock.mock.calls[0][0]({ model: voiceModel.name, downloaded: 45, total: 100, progress: 45 }));
    expect(within(row).getByText("45%")).toBeInTheDocument();
    expect(within(row).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "45");
    expect(within(row).queryByText("model.mlx.installingRuntime")).toBeNull();
  });

  it("switches between installed MLX choices with a single backend model update", async () => {
    getModelsMock.mockResolvedValue(mlxModels.map(model => ({ ...model, downloaded: true, runtime_ready: true })));
    render(<ModelSettings />);

    const parakeet = await screen.findByRole("article", { name: "MLX Parakeet v3" });
    fireEvent.click(within(parakeet).getByRole("button", { name: "model.catalog.use" }));
    await waitFor(() => expect(updateSettingContextMock).toHaveBeenCalledExactlyOnceWith("model", "mlx-parakeet-v3"));
  });

  it("shows backend runtime installation as indeterminate before model byte progress", async () => {
    render(<ModelSettings />);
    const row = await screen.findByRole("article", { name: "MLX Whisper Turbo" });
    fireEvent.click(within(row).getByRole("button", { name: "model.available.download" }));
    await waitFor(() => expect(onModelDownloadProgressMock).toHaveBeenCalledTimes(1));
    const progress = onModelDownloadProgressMock.mock.calls[0][0];

    await act(async () => progress({ model: "mlx-whisper-turbo", downloaded: 0, total: 0, progress: 0, stage: "runtime" }));
    expect(within(row).getByText("model.mlx.installingRuntime")).toBeInTheDocument();
    expect(within(row).getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
    expect(within(row).queryByText("0%")).toBeNull();

    await act(async () => progress({ model: "mlx-whisper-turbo", downloaded: 25, total: 100, progress: 25, stage: "model" }));
    expect(within(row).queryByText("model.mlx.installingRuntime")).toBeNull();
    expect(within(row).getByText("25%")).toBeInTheDocument();
    expect(within(row).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "25");
  });

  it("clears runtime stage after cancellation and preserves legacy numeric events", async () => {
    render(<ModelSettings />);
    const row = await screen.findByRole("article", { name: "MLX Whisper Turbo" });
    fireEvent.click(within(row).getByRole("button", { name: "model.available.download" }));
    await waitFor(() => expect(onModelDownloadProgressMock).toHaveBeenCalledTimes(1));
    const progress = onModelDownloadProgressMock.mock.calls[0][0];

    await act(async () => progress({ model: "mlx-whisper-turbo", downloaded: 0, total: 0, progress: 0, stage: "runtime" }));
    expect(within(row).getByText("model.mlx.installingRuntime")).toBeInTheDocument();
    fireEvent.click(within(row).getByRole("button", { name: "model.available.cancel" }));
    await waitFor(() => expect(cancelDownloadMock).toHaveBeenCalledWith("mlx-whisper-turbo"));
    await act(async () => onModelDownloadCancelledMock.mock.calls[0][0]("mlx-whisper-turbo"));
    fireEvent.click(within(row).getByRole("button", { name: "model.available.download" }));
    expect(within(row).queryByText("model.mlx.installingRuntime")).toBeNull();

    await act(async () => progress({ model: "mlx-whisper-turbo", downloaded: 40, total: 100, progress: 40 }));
    expect(within(row).getByText("40%")).toBeInTheDocument();
  });

  it("clears runtime installation after failure so retry does not show a stale stage", async () => {
    let rejectDownload: ((error: Error) => void) | undefined;
    downloadModelMock.mockImplementationOnce(() => new Promise<void>((_resolve, reject) => {
      rejectDownload = reject;
    }));
    render(<ModelSettings />);
    const row = await screen.findByRole("article", { name: "MLX Whisper Turbo" });
    fireEvent.click(within(row).getByRole("button", { name: "model.available.download" }));
    await waitFor(() => expect(onModelDownloadProgressMock).toHaveBeenCalledTimes(1));

    await act(async () => onModelDownloadProgressMock.mock.calls[0][0]({ model: "mlx-whisper-turbo", downloaded: 0, total: 0, progress: 0, stage: "runtime" }));
    expect(within(row).getByText("model.mlx.installingRuntime")).toBeInTheDocument();
    await act(async () => rejectDownload?.(new Error("MLX runtime installation failed")));
    expect(showErrorToastMock).toHaveBeenCalledWith("MLX runtime installation failed");
    fireEvent.click(within(row).getByRole("button", { name: "model.available.download" }));
    expect(within(row).queryByText("model.mlx.installingRuntime")).toBeNull();
  });

  it("refreshes backend completeness after download and clears installation presentation", async () => {
    render(<ModelSettings />);
    const row = await screen.findByRole("article", { name: "MLX Whisper Turbo" });
    fireEvent.click(within(row).getByRole("button", { name: "model.available.download" }));
    await waitFor(() => expect(onModelDownloadProgressMock).toHaveBeenCalledTimes(1));
    await act(async () => onModelDownloadProgressMock.mock.calls[0][0]({ model: "mlx-whisper-turbo", downloaded: 0, total: 0, progress: 0, stage: "runtime" }));
    getModelsMock.mockResolvedValue(mlxModels.map(model => ({ ...model, runtime_ready: true, downloaded: true })));

    await act(async () => onModelDownloadCompleteMock.mock.calls[0][0]("mlx-whisper-turbo"));
    expect(within(row).queryByText("model.mlx.installingRuntime")).toBeNull();
    expect(within(row).queryByRole("progressbar")).toBeNull();
    expect(within(row).getByRole("button", { name: "model.catalog.use" })).toBeInTheDocument();
    expect(updateSettingContextMock).not.toHaveBeenCalled();
  });

  it("waits for backend completeness instead of treating runtime presence as a usable model", async () => {
    getModelsMock.mockResolvedValue([{ ...mlxModels[0], runtime_ready: true, downloaded: false }]);
    render(<ModelSettings />);

    const row = await screen.findByRole("article", { name: "MLX Whisper Turbo" });
    expect(within(row).getByRole("button", { name: "model.available.download" })).toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: "model.catalog.use" })).toBeNull();
  });
});
