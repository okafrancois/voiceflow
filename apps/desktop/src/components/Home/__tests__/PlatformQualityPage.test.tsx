import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/ui/confirm";
import type { DiagnosticReport, QualitySummary } from "@/lib/tauri";
import { PlatformQualityPage } from "../PlatformQualityPage";

const {
  applyPresetMock,
  clearCodeContextMock,
  clearMetricsMock,
  exportMetricsMock,
  getCodeContextMock,
  getEventsMock,
  getSummaryMock,
  runDiagnosticsMock,
  runLatencyTestMock,
  selectMediaFileMock,
  setCodeContextMock,
} = vi.hoisted(() => ({
  applyPresetMock: vi.fn(),
  clearCodeContextMock: vi.fn(),
  clearMetricsMock: vi.fn(),
  exportMetricsMock: vi.fn(),
  getCodeContextMock: vi.fn(),
  getEventsMock: vi.fn(),
  getSummaryMock: vi.fn(),
  runDiagnosticsMock: vi.fn(),
  runLatencyTestMock: vi.fn(),
  selectMediaFileMock: vi.fn(),
  setCodeContextMock: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  historyCommands: { selectMediaFile: selectMediaFileMock },
  platformQualityCommands: {
    applyPreset: applyPresetMock,
    clearCodeContext: clearCodeContextMock,
    clearMetrics: clearMetricsMock,
    exportMetrics: exportMetricsMock,
    getCodeContext: getCodeContextMock,
    getEvents: getEventsMock,
    getSummary: getSummaryMock,
    runDiagnostics: runDiagnosticsMock,
    runLatencyTest: runLatencyTestMock,
    setCodeContext: setCodeContextMock,
  },
}));

vi.mock("@/lib/toast", () => ({
  showErrorToast: vi.fn(),
  showToast: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const summary: QualitySummary = {
  total_transcriptions: 4,
  transcription_failures: 1,
  injection_failures: 2,
  corrections: 1,
  correction_rate_percent: 25,
  local_transcriptions: 3,
  cloud_transcriptions: 2,
  stt_latency_ms: { p50: 100, p95: 300 },
  polish_latency_ms: { p50: 20, p95: 50 },
  total_latency_ms: { p50: 120, p95: 350 },
  application_injection_failures: { "com.example.editor": 2 },
};

const diagnostics: DiagnosticReport = {
  microphone: {
    ready: true,
    device_name: "Built-in microphone",
    sample_rate_hz: 48_000,
    channels: 1,
    peak_level: 0.25,
    error: null,
  },
  hardware: {
    total_memory_mb: 16_384,
    logical_cpu_count: 8,
    architecture: "arm64",
  },
  recommended_model: {
    model_name: "whisper-turbo",
    reason: "fits hardware",
  },
  recommended_preset: "local_only",
  recommendation_reason: "works locally",
  latency: null,
};

function renderPage() {
  return render(<ConfirmProvider><PlatformQualityPage /></ConfirmProvider>);
}

describe("PlatformQualityPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSummaryMock.mockResolvedValue(summary);
    getEventsMock.mockResolvedValue([]);
    getCodeContextMock.mockResolvedValue({ editor_id: "com.microsoft.VSCode" });
    runDiagnosticsMock.mockResolvedValue(diagnostics);
    applyPresetMock.mockResolvedValue({});
    setCodeContextMock.mockImplementation(async (context) => context);
    clearCodeContextMock.mockResolvedValue(undefined);
    clearMetricsMock.mockResolvedValue(3);
    exportMetricsMock.mockResolvedValue("/tmp/quality.json");
    selectMediaFileMock.mockResolvedValue(null);
    runLatencyTestMock.mockResolvedValue(null);
  });

  it("runs diagnostics and applies a backend preset", async () => {
    renderPage();
    fireEvent.click(screen.getByRole("button", { name: "platformQuality.diagnostics.run" }));
    expect(await screen.findByTestId("diagnostic-report")).toHaveTextContent("whisper-turbo");
    expect(runDiagnosticsMock).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "platformQuality.presets.localOnly" }));
    await waitFor(() => expect(applyPresetMock).toHaveBeenCalledWith("local_only"));
  });

  it("sends period, source, outcome, and application filters to the backend", async () => {
    renderPage();
    await waitFor(() => expect(getSummaryMock).toHaveBeenCalled());
    expect(screen.getByTestId("application-injection-failures")).toHaveTextContent("com.example.editor");
    expect(screen.getByTestId("application-injection-failures")).toHaveTextContent("2");

    fireEvent.change(screen.getByLabelText("platformQuality.filters.period"), { target: { value: "days_7" } });
    fireEvent.change(screen.getByLabelText("platformQuality.filters.source"), { target: { value: "cloud" } });
    fireEvent.change(screen.getByLabelText("platformQuality.filters.outcome"), { target: { value: "transcription_failure" } });
    fireEvent.change(screen.getByLabelText("platformQuality.filters.application"), { target: { value: "com.example.browser" } });

    await waitFor(() => expect(getSummaryMock).toHaveBeenLastCalledWith(expect.objectContaining({
      application_id: "com.example.browser",
      is_cloud: true,
      kind: "transcription_failure",
    })));
  });

  it("round-trips the typed editor_id code context", async () => {
    renderPage();
    const editor = await screen.findByLabelText("platformQuality.code.editor");
    expect(editor).toHaveValue("com.microsoft.VSCode");
    fireEvent.change(editor, { target: { value: "dev.zed.Zed" } });
    fireEvent.click(screen.getByRole("button", { name: "common.save" }));

    await waitFor(() => expect(setCodeContextMock).toHaveBeenCalledWith(expect.objectContaining({
      editor_id: "dev.zed.Zed",
    })));
  });

  it("requires confirmation before clearing metrics or overwriting an export", async () => {
    exportMetricsMock
      .mockRejectedValueOnce("Quality export destination already exists")
      .mockResolvedValueOnce("/tmp/quality.json");
    renderPage();

    fireEvent.click(screen.getByRole("button", { name: "platformQuality.clear.button" }));
    fireEvent.click(await screen.findByRole("button", { name: "platformQuality.clear.confirm" }));
    await waitFor(() => expect(clearMetricsMock).toHaveBeenCalledTimes(1));

    fireEvent.change(screen.getByLabelText("platformQuality.export.path"), { target: { value: "/tmp/quality.json" } });
    fireEvent.click(screen.getByRole("button", { name: "platformQuality.export.button" }));
    fireEvent.click(await screen.findByRole("button", { name: "platformQuality.export.overwrite" }));
    await waitFor(() => expect(exportMetricsMock).toHaveBeenLastCalledWith(
      "/tmp/quality.json",
      expect.objectContaining({ application_id: null }),
      true,
    ));
  });
});
