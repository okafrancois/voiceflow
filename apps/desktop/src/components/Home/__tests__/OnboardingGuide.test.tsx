import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RecommendedModel, events } from "@/lib/tauri";
import { OnboardingGuide, OnboardingPresetControl } from "../OnboardingGuide";

const mocks = vi.hoisted(() => {
  const onModelDownloadProgressMock = vi.fn<typeof events.onModelDownloadProgress>(async () => () => undefined);
  const onModelDownloadCompleteMock = vi.fn<typeof events.onModelDownloadComplete>(async () => () => undefined);

  return {
    applyPermissionMock: vi.fn(async () => undefined),
    applyPresetMock: vi.fn(async () => undefined),
    checkPermissionMock: vi.fn(async () => "granted"),
    downloadModelMock: vi.fn(async () => undefined),
    isModelDownloadedMock: vi.fn(async () => false),
    onAudioLevelMock: vi.fn(async () => () => undefined),
    onHotkeyCapturedMock: vi.fn(async () => () => undefined),
    onModelDownloadCompleteMock,
    onModelDownloadProgressMock,
    onRecordingStateChangedMock: vi.fn(async () => () => undefined),
    onTranscriptionCompleteMock: vi.fn(async () => () => undefined),
    onTranscriptionErrorMock: vi.fn(async () => () => undefined),
    recommendModelsByLanguageMock: vi.fn<(language: string) => Promise<RecommendedModel[]>>(async () => [
      {
        engine_type: "sensevoice",
        model_name: "sense-voice-small",
        display_name: "SenseVoice Small",
        size_mb: 229,
        speed_score: 4,
        accuracy_score: 4,
        downloaded: false,
      },
    ]),
    startRecordingMock: vi.fn(async () => undefined),
    stopRecordingMock: vi.fn(async () => undefined),
    trackMock: vi.fn(),
    updateSettingMock: vi.fn<(key: string, value: unknown) => Promise<void>>(async () => undefined),
  };
});

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, string>) => (
      params?.hotkey ? `${key} ${params.hotkey}` : key
    ),
  }),
}));

vi.mock("@/contexts/SettingsContext", () => ({
  useSettingsContext: () => ({
    loading: false,
    polishAvailable: false,
    settings: {
      workflow_profiles: [{ id: "dictate", name: "Dictate", hotkey: "Shift+Space", trigger_mode: "hold", polish_template_id: null, language: null, translation_target: null, output_action: "insert", code_aware: false, protected: true }],
      stt_engine_language: "zh-CN",
    },
    updateSetting: mocks.updateSettingMock,
  }),
}));

vi.mock("@/lib/analytics", () => ({
  analytics: {
    track: mocks.trackMock,
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: {
    error: vi.fn(),
  },
}));

vi.mock("@/lib/toast", () => ({
  showErrorToast: vi.fn(),
  showToast: vi.fn(),
}));

vi.mock("@/components/ui/hotkey-input", () => ({
  HotkeyInput: () => <button type="button">hotkey</button>,
  formatHotkey: (hotkey: string) => hotkey,
}));

vi.mock("@/lib/tauri", () => ({
  platformQualityCommands: {
    applyPreset: mocks.applyPresetMock,
  },
  audioCommands: {
    startRecording: mocks.startRecordingMock,
    stopRecording: mocks.stopRecordingMock,
  },
  events: {
    onAudioLevel: mocks.onAudioLevelMock,
    onHotkeyCaptured: mocks.onHotkeyCapturedMock,
    onModelDownloadComplete: mocks.onModelDownloadCompleteMock,
    onModelDownloadProgress: mocks.onModelDownloadProgressMock,
    onRecordingStateChanged: mocks.onRecordingStateChangedMock,
    onTranscriptionComplete: mocks.onTranscriptionCompleteMock,
    onTranscriptionError: mocks.onTranscriptionErrorMock,
  },
  modelCommands: {
    downloadModel: mocks.downloadModelMock,
    isModelDownloaded: mocks.isModelDownloadedMock,
    recommendModelsByLanguage: mocks.recommendModelsByLanguageMock,
  },
  systemCommands: {
    applyPermission: mocks.applyPermissionMock,
    checkPermission: mocks.checkPermissionMock,
  },
}));

describe("OnboardingGuide model download flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.downloadModelMock.mockImplementation(async () => undefined);
    mocks.checkPermissionMock.mockResolvedValue("granted");
    mocks.isModelDownloadedMock.mockResolvedValue(false);
    mocks.onModelDownloadCompleteMock.mockImplementation(async () => () => undefined);
    mocks.onModelDownloadProgressMock.mockImplementation(async () => () => undefined);
  });

  async function advanceToModelStep() {
    const modal = screen.getByTestId("onboarding-modal");
    const nextButton = screen.getByTestId("onboarding-primary-action");
    for (const step of ["language", "hotkey", "model"]) {
      fireEvent.click(nextButton);
      await waitFor(() => expect(modal).toHaveAttribute("data-step-id", step));
    }
    return nextButton;
  }

  it.each([
    { engine_type: "mlx", model_name: "mlx-whisper-turbo", display_name: "MLX Whisper Turbo" },
    { engine_type: "whisper", model_name: "whisper-base", display_name: "Whisper Base" },
  ])("uses the backend $engine_type recommendation and saves only the model", async recommendation => {
    mocks.recommendModelsByLanguageMock.mockResolvedValueOnce([{
      ...recommendation,
      size_mb: 500,
      speed_score: 0,
      accuracy_score: 0,
      downloaded: true,
    }]);
    mocks.isModelDownloadedMock.mockResolvedValue(true);
    render(<OnboardingGuide isOpen onClose={vi.fn()} />);

    const nextButton = await advanceToModelStep();
    await waitFor(() => expect(mocks.isModelDownloadedMock).toHaveBeenCalledWith(recommendation.model_name));
    expect(screen.getByText(recommendation.display_name)).toBeInTheDocument();
    await waitFor(() => expect(nextButton).toBeEnabled());
    fireEvent.click(nextButton);

    await waitFor(() => expect(mocks.updateSettingMock).toHaveBeenCalledWith("model", recommendation.model_name));
    expect(mocks.updateSettingMock.mock.calls.some(([key]) => key === "stt_engine")).toBe(false);
    expect(mocks.downloadModelMock).not.toHaveBeenCalled();
  });

  it("shows installation progress without infrastructure copy or premature completion", async () => {
    mocks.recommendModelsByLanguageMock.mockResolvedValueOnce([{
      engine_type: "mlx",
      model_name: "mlx-whisper-turbo",
      display_name: "MLX Whisper Turbo",
      size_mb: 500,
      speed_score: 0,
      accuracy_score: 0,
      downloaded: false,
    }]);
    render(<OnboardingGuide isOpen onClose={vi.fn()} />);
    const nextButton = await advanceToModelStep();
    await waitFor(() => expect(mocks.downloadModelMock).toHaveBeenCalledWith("mlx-whisper-turbo"));
    const progress = mocks.onModelDownloadProgressMock.mock.calls[0][0];
    await act(async () => progress({ model: "mlx-whisper-turbo", downloaded: 0, total: 0, progress: 100, stage: "runtime" }));

    expect(screen.getByText("model.mlx.installingRuntime")).toBeInTheDocument();
    expect(screen.queryByText("model.mlx.managedDescription")).toBeNull();
    expect(nextButton).toBeDisabled();
    await act(async () => progress({ model: "mlx-whisper-turbo", downloaded: 100, total: 100, progress: 100, stage: "model" }));
    expect(screen.queryByText("model.mlx.installingRuntime")).toBeNull();
    expect(nextButton).toBeDisabled();

    mocks.recommendModelsByLanguageMock.mockResolvedValueOnce([{
      engine_type: "mlx",
      model_name: "mlx-whisper-turbo",
      display_name: "MLX Whisper Turbo",
      size_mb: 500,
      speed_score: 0,
      accuracy_score: 0,
      downloaded: true,
    }]);
    await act(async () => mocks.onModelDownloadCompleteMock.mock.calls[0][0]("mlx-whisper-turbo"));
    await waitFor(() => expect(nextButton).toBeEnabled());
  });

  it("does not invent a native fallback when the backend returns no recommendations", async () => {
    mocks.recommendModelsByLanguageMock.mockResolvedValueOnce([]);
    render(<OnboardingGuide isOpen onClose={vi.fn()} />);
    const nextButton = await advanceToModelStep();
    await waitFor(() => expect(mocks.recommendModelsByLanguageMock).toHaveBeenCalled());
    expect(mocks.isModelDownloadedMock).not.toHaveBeenCalled();
    expect(mocks.downloadModelMock).not.toHaveBeenCalled();
    expect(nextButton).toBeDisabled();
  });

  it("starts the model download from the model step after progress listeners are mounted", async () => {
    let progressListenerReady = false;
    let releaseProgressListener: (() => void) | undefined;
    mocks.onModelDownloadProgressMock.mockImplementationOnce(() => (
      new Promise<() => undefined>((resolve) => {
        releaseProgressListener = () => {
          progressListenerReady = true;
          resolve(() => undefined);
        };
      })
    ));
    mocks.downloadModelMock.mockImplementationOnce(async () => {
      expect(progressListenerReady).toBe(true);
    });

    render(<OnboardingGuide isOpen onClose={vi.fn()} />);

    const modal = screen.getByTestId("onboarding-modal");
    const nextButton = screen.getByTestId("onboarding-primary-action");

    expect(modal).toHaveAttribute("data-step-id", "permissions");

    fireEvent.click(nextButton);
    await waitFor(() => expect(modal).toHaveAttribute("data-step-id", "language"));

    fireEvent.click(nextButton);
    await waitFor(() => expect(modal).toHaveAttribute("data-step-id", "hotkey"));
    await Promise.resolve();

    expect(mocks.isModelDownloadedMock).not.toHaveBeenCalled();
    expect(mocks.downloadModelMock).not.toHaveBeenCalled();

    fireEvent.click(nextButton);
    await waitFor(() => expect(modal).toHaveAttribute("data-step-id", "model"));
    await Promise.resolve();

    expect(mocks.isModelDownloadedMock).not.toHaveBeenCalled();
    expect(mocks.downloadModelMock).not.toHaveBeenCalled();
    expect(releaseProgressListener).toBeDefined();

    releaseProgressListener?.();

    await waitFor(() => {
      expect(mocks.downloadModelMock).toHaveBeenCalledWith("sense-voice-small");
    });

    expect(mocks.onModelDownloadProgressMock).toHaveBeenCalledTimes(1);
    expect(mocks.onModelDownloadCompleteMock).toHaveBeenCalledTimes(1);
  });

  it("lets users apply a setup preset before leaving onboarding", async () => {
    render(<OnboardingPresetControl />);

    fireEvent.click(screen.getByRole("button", { name: "platformQuality.presets.localOnly" }));

    await waitFor(() => expect(mocks.applyPresetMock).toHaveBeenCalledWith("local_only"));
  });
});
