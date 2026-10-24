import { useTranslation } from "react-i18next";
import type { ModelInfo } from "@/lib/tauri";
import { logger } from "@/lib/logger";
import { showErrorToast } from "@/lib/toast";
import { analytics } from "@/lib/analytics";
import { AnalyticsEvents } from "@/lib/events";
import { useSettingsContext } from "@/contexts/SettingsContext";
import { ActiveModelSummary, ModelCatalog, displayModelName, displaySize } from "@/components/Home/model/ModelCatalog";

interface VoiceInputSectionProps {
  models: ModelInfo[];
  downloading: Set<string>;
  downloadProgress: Record<string, number>;
  onDownload: (modelName: string) => void;
  onCancel: (modelName: string) => void;
  onDelete: (modelName: string) => void;
}

export function VoiceInputSection({ models, downloading, downloadProgress, onDownload, onCancel, onDelete }: VoiceInputSectionProps) {
  const { t, i18n } = useTranslation();
  const { settings, updateSetting } = useSettingsContext();
  if (!settings) return null;

  const selectedModel = models.find(model => model.name === settings.model);
  const serviceEnabled = settings.cloud_stt_enabled;
  const provider = settings.active_cloud_stt_provider;
  const service = settings.cloud_stt_configs[provider];
  const activeName = serviceEnabled
    ? `${provider === "mlx-audio" ? "MLX Audio" : provider} · ${service?.model || t("model.catalog.configuredService")}`
    : selectedModel ? displayModelName(selectedModel.display_name) : t("model.active.noModels");

  const handleModelChange = async (value: string) => {
    try {
      await updateSetting("model", value);
      if (serviceEnabled) await updateSetting("cloud_stt_enabled", false);
      analytics.track(AnalyticsEvents.SETTING_CHANGED, { setting: "model", value });
    } catch (error) {
      logger.error("failed_to_select_transcription_model", { error: String(error) });
      showErrorToast(String(error));
    }
  };

  return (
    <div className="model-picker rounded-2xl border border-border bg-card p-5 md:p-6">
      <ActiveModelSummary name={activeName} ready={serviceEnabled || Boolean(selectedModel?.downloaded)} detail={serviceEnabled ? provider === "mlx-audio" ? t("model.catalog.localService") : t("model.catalog.speechService") : selectedModel?.downloaded ? t("model.info.ready") : t("model.info.notDownloaded")} />
      <ModelCatalog
        kind="transcription"
        language={settings.stt_engine_language}
        models={models.map(model => ({
          id: model.name,
          name: model.display_name,
          description: model.built_in ? t("model.hint.appleStreaming") : model.language_codes?.length ? t("model.hint.cjkBest") : t("model.hint.multiLang"),
          sizeMb: model.built_in ? 0 : model.size_mb,
          sizeLabel: model.built_in ? t("model.available.builtIn") : displaySize(model.size_mb, i18n.language),
          available: model.downloaded,
          builtIn: model.built_in,
          selected: !serviceEnabled && model.name === settings.model,
          speed: { value: model.speed_score, maximum: 10 },
          accuracy: { value: model.accuracy_score, maximum: 10 },
          languageCodes: model.language_codes,
          downloading: downloading.has(model.name),
          progress: downloadProgress[model.name],
        }))}
        onSelect={id => { void handleModelChange(id); }}
        onDownload={onDownload}
        onCancel={onCancel}
        onDelete={onDelete}
      />
    </div>
  );
}
