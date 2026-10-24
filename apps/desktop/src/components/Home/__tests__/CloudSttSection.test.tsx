import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CloudSttSection } from "../cloud/CloudSttSection";
import type { CloudProviderSchemas } from "@/lib/tauri";

const { getSchemas, checkConnection, updateSetting } = vi.hoisted(() => ({
  getSchemas: vi.fn(),
  checkConnection: vi.fn(),
  updateSetting: vi.fn(),
}));

vi.mock("@/contexts/SettingsContext", () => ({
  useSettingsContext: () => ({
    settings: {
      active_cloud_stt_provider: "mlx-audio",
      cloud_stt_enabled: true,
      cloud_stt_configs: {},
    },
    updateSetting,
  }),
}));

vi.mock("@/lib/tauri", () => ({
  settingsCommands: {
    getCloudProviderSchemas: getSchemas,
    checkActiveCloudSttConfig: checkConnection,
  },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const schemas: CloudProviderSchemas = {
  stt: [{
    id: "mlx-audio",
    name: "MLX Audio (Local)",
    fields: [
      { name: "Base URL", key: "base_url", required: false, default_value: "http://127.0.0.1:8000/v1", example: "http://127.0.0.1:8000/v1", secret: false },
      { name: "Model", key: "model", required: false, default_value: "mlx-community/whisper-large-v3-turbo-asr-fp16", example: "mlx-community/whisper-large-v3-turbo-asr-fp16", secret: false },
    ],
  }],
  polish: [],
};

describe("MLX Audio speech service", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSchemas.mockResolvedValue(schemas);
    checkConnection.mockResolvedValue({ ok: true, kind: "ok", message: "Local server reachable", duration_ms: 5 });
  });

  it("explains local processing and offers endpoint and model without credentials", async () => {
    render(<CloudSttSection />);
    expect(await screen.findByPlaceholderText("http://127.0.0.1:8000/v1")).toBeInTheDocument();
    expect(screen.getByText("cloud.stt.local.description")).toBeInTheDocument();
    expect(screen.getByText("cloud.stt.local.serverHint")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("mlx-community/whisper-large-v3-turbo-asr-fp16")).toBeInTheDocument();
    expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
    expect(screen.queryByText("cloud.stt.providerGuide")).not.toBeInTheDocument();
  });

  it("checks the local server with defaults and no API key", async () => {
    render(<CloudSttSection />);
    const button = await screen.findByRole("button", { name: "cloud.check.button" });
    fireEvent.click(button);
    await waitFor(() => expect(checkConnection).toHaveBeenCalledOnce());
    expect(await screen.findByText("cloud.check.ok")).toBeInTheDocument();
  });
});
