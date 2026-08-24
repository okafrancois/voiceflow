import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GeneralSettings } from "../GeneralSettings";
import type { AppSettings } from "@/lib/tauri";

const {
  getAudioDevicesMock,
  getIdentifierMock,
  getPlatformMock,
  getRetentionStatusMock,
  updateSettingMock,
} = vi.hoisted(() => ({
  getAudioDevicesMock: vi.fn(),
  getIdentifierMock: vi.fn(),
  getPlatformMock: vi.fn(),
  getRetentionStatusMock: vi.fn(),
  updateSettingMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getIdentifier: getIdentifierMock,
}));

vi.mock("@/contexts/SettingsContext", () => ({
  useSettingsContext: () => ({
    settings: testSettings,
    loading: false,
    polishAvailable: false,
    updateSetting: updateSettingMock,
  }),
}));

vi.mock("@/lib/tauri", () => ({
  settingsCommands: {
    getAvailableSubdomains: vi.fn(async () => []),
    clearCorrectionMemory: vi.fn(async () => undefined),
    openCorrectionMemoryDirectory: vi.fn(async () => undefined),
  },
  systemCommands: {
    getAudioDevices: getAudioDevicesMock,
    getPlatform: getPlatformMock,
  },
  historyCommands: {
    getRetentionStatus: getRetentionStatusMock,
  },
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

vi.mock("@/lib/toast", () => ({
  showErrorToast: vi.fn(),
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
    t: (key: string) => key,
  }),
}));

const testSettings: AppSettings = {
  active_cloud_polish_provider: "openai",
  active_cloud_stt_provider: "volcengine",
  analytics_opt_in: false,
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
  polish_model: "",
  polish_stream_direct_typing_enabled: false,
  polish_system_prompt: "",
  recording_mode: "hold",
  shortcut_profiles: {
    dictate: {
      hotkey: "Cmd+Slash",
      trigger_mode: "hold",
      action: { Record: { polish_template_id: null } },
    },
    riff: {
      hotkey: "Opt+Slash",
      trigger_mode: "toggle",
      action: { Record: { polish_template_id: null } },
    },
  },
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

describe("GeneralSettings correction memory directory entry", () => {
  beforeEach(() => {
    getAudioDevicesMock.mockResolvedValue(["default"]);
    getPlatformMock.mockResolvedValue("macos");
    getIdentifierMock.mockReset();
    getRetentionStatusMock.mockResolvedValue({ text_entries: 0, audio_files: 0, audio_bytes: 0 });
    updateSettingMock.mockReset();
  });

  it("hides the correction memory folder button outside in-house builds", async () => {
    getIdentifierMock.mockResolvedValue("com.voiceflow.voicetotext");
    getRetentionStatusMock.mockResolvedValue({ text_entries: 3, audio_files: 0, audio_bytes: 0 });

    render(<GeneralSettings />);

    await waitFor(() => {
      expect(getIdentifierMock).toHaveBeenCalled();
    });

    expect(screen.queryByText("general.privacy.correctionMemoryOpenAction")).not.toBeInTheDocument();
  });

  it("shows the correction memory folder button in in-house builds", async () => {
    getIdentifierMock.mockResolvedValue("com.voiceflow.voicetotext.inhouse");

    render(<GeneralSettings />);

    expect(await screen.findByText("general.privacy.correctionMemoryOpenAction")).toBeInTheDocument();
  });
});

describe("GeneralSettings retention controls", () => {
  beforeEach(() => {
    getAudioDevicesMock.mockResolvedValue(["default"]);
    getPlatformMock.mockResolvedValue("macos");
    getIdentifierMock.mockResolvedValue("com.voiceflow.voicetotext");
    updateSettingMock.mockReset();
  });

  it("shows independent text and audio retention selectors", async () => {
    render(<GeneralSettings />);

    expect(await screen.findByText("general.privacy.retentionStatus")).toBeInTheDocument();
    expect(screen.getByLabelText("general.privacy.textRetention")).toHaveValue("days_90");
    expect(screen.getByLabelText("general.privacy.audioRetention")).toHaveValue("never");
  });

  it("persists each retention policy through its backend setting key", async () => {
    render(<GeneralSettings />);

    fireEvent.click(screen.getByLabelText("general.privacy.textRetention"));
    fireEvent.click(screen.getByText("general.privacy.retention.days30"));
    fireEvent.click(screen.getByLabelText("general.privacy.audioRetention"));
    fireEvent.click(screen.getByText("general.privacy.retention.days7"));

    await waitFor(() => {
      expect(updateSettingMock).toHaveBeenCalledWith("text_retention", "days_30");
      expect(updateSettingMock).toHaveBeenCalledWith("audio_retention", "days_7");
    });
  });
});
