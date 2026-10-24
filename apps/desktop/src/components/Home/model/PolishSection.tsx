import { useEffect, useState } from "react";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Check, CircleNotch } from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  modelCommands,
  settingsCommands,
  type CloudConnectionCheckResult,
  type LocalPolishRuntimeSettings,
  type PolishModelInfo,
  type PolishModelStatus,
} from "@/lib/tauri";
import { logger } from "@/lib/logger";
import { showErrorToast } from "@/lib/toast";
import { ActiveModelSummary, ModelCatalog, TEMPLATE_LABEL_KEYS, parseDisplaySize, displaySize } from "@/components/Home/model/ModelCatalog";
import { analytics } from "@/lib/analytics";
import { AnalyticsEvents } from "@/lib/events";
import { useSettingsContext } from "@/contexts/SettingsContext";

const LATENCY_LABEL_KEYS: Record<PolishModelInfo["latency_profile"]["class"], string> = {
  fast: "model.polish.latency.fast",
  balanced: "model.polish.latency.balanced",
  slow: "model.polish.latency.slow",
  heavy: "model.polish.latency.heavy",
};

const DEFAULT_LOCAL_RUNTIME: LocalPolishRuntimeSettings = {
  provider_type: "llama-server",
  base_url: "http://127.0.0.1:8000/v1",
  api_key: "",
  server_command: "",
  server_args_json: "",
  ready_timeout_secs: 20,
};

interface PolishSectionProps {
  polishModels: PolishModelInfo[];
  selectedPolishModel: string;
  setSelectedPolishModel: (id: string) => void;
  polishDownloadingId: string | null;
  polishProgress: number | null;
  onDownload: (modelId: string) => void;
  onCancel: (modelId: string) => void;
  onDelete: (modelId: string) => void;
}

function formatMemory(mb: number | null | undefined): string | null {
  if (!mb) return null;
  if (mb >= 1024) {
    const value = mb / 1024;
    return `${Number.isInteger(value) ? value : value.toFixed(1)}GB`;
  }
  return `${mb}MB`;
}

function getCompatibilityWarning(model: PolishModelInfo | undefined, t: TFunction): string | null {
  const compatibility = model?.compatibility;
  if (!compatibility || compatibility.level === "smooth") return null;

  const deviceMemory =
    formatMemory(compatibility.device_memory_mb) ?? t("model.polish.compat.unknownMemory");
  const minimumMemory = formatMemory(compatibility.minimum_memory_mb) ?? "";
  const recommendedMemory = formatMemory(compatibility.recommended_memory_mb) ?? "";

  if (compatibility.code === "memory_below_minimum") {
    return t("model.polish.compat.memoryBelowMinimum", {
      deviceMemory,
      minimumMemory,
    });
  }

  if (compatibility.code === "memory_below_recommended") {
    return t("model.polish.compat.memoryBelowRecommended", {
      deviceMemory,
      recommendedMemory,
    });
  }

  if (compatibility.code === "cpu_threads_low") {
    return t("model.polish.compat.cpuThreadsLow", {
      cpuThreads: compatibility.logical_cpu_count,
    });
  }

  return t("model.polish.compat.memoryUnknown", {
    recommendedMemory,
  });
}

function getTemplateNames(templateIds: string[], t: TFunction): string {
  return templateIds
    .map((id) => t(TEMPLATE_LABEL_KEYS[id] ?? id))
    .join(" · ");
}

function getLatencyLabel(model: PolishModelInfo, t: TFunction): string {
  return t(LATENCY_LABEL_KEYS[model.latency_profile.class]);
}

function getLatencySummary(model: PolishModelInfo, t: TFunction): string {
  return t("model.polish.latency.bestFor", {
    templates: getTemplateNames(model.latency_profile.recommended_templates, t),
  });
}

function getLatencyCaution(model: PolishModelInfo, t: TFunction): string | null {
  if (model.latency_profile.caution_templates.length === 0) return null;

  return t("model.polish.latency.slowerFor", {
    templates: getTemplateNames(model.latency_profile.caution_templates, t),
  });
}

function getLocalRuntimeProviderLabel(providerType: string, t: TFunction): string {
  switch (providerType) {
    case "llama-server":
      return t("model.polish.localRuntime.llamaServer");
    case "lm-studio":
      return t("model.polish.localRuntime.lmStudio");
    case "ollama":
      return t("model.polish.localRuntime.ollama");
    case "custom":
      return t("model.polish.localRuntime.custom");
    default:
      return providerType;
  }
}

function getRuntimeCheckMessage(
  result: CloudConnectionCheckResult | null,
  checking: boolean,
  t: TFunction,
): string | null {
  if (checking) return t("cloud.check.checking");
  if (!result) return null;

  switch (result.kind) {
    case "ok":
      return t("cloud.check.ok");
    case "disabled":
      return t("cloud.check.disabled");
    case "missing_required":
      return t("cloud.check.missing_required");
    case "invalid_url":
      return t("cloud.check.invalid_url");
    case "unsupported_provider":
      return t("cloud.check.unsupported_provider");
    case "auth_failed":
      return t("cloud.check.auth_failed");
    case "model_failed":
      return t("cloud.check.model_failed");
    case "network_failed":
      return t("cloud.check.network_failed");
    case "timeout":
      return t("cloud.check.timeout");
    case "provider_error":
      return t("cloud.check.provider_error");
  }
}

export function PolishSection({
  polishModels,
  selectedPolishModel,
  setSelectedPolishModel,
  polishDownloadingId,
  polishProgress,
  onDownload,
  onCancel,
  onDelete,
}: PolishSectionProps) {
  const { t, i18n } = useTranslation();
  const { settings, updateSetting } = useSettingsContext();
  const [runtimeCheckResult, setRuntimeCheckResult] = useState<CloudConnectionCheckResult | null>(null);
  const [checkingRuntime, setCheckingRuntime] = useState(false);
  const [polishModelStatus, setPolishModelStatus] = useState<PolishModelStatus | null>(null);

  const localRuntime = settings?.local_polish_runtime ?? DEFAULT_LOCAL_RUNTIME;

  const refreshPolishModelStatus = async () => {
    try {
      const status = await modelCommands.getPolishModelStatus();
      setPolishModelStatus(status);
    } catch (err) {
      logger.error("failed_to_load_polish_model_status", { error: String(err) });
      setPolishModelStatus(null);
    }
  };

  const handlePolishModelSelect = async (modelId: string) => {
    try {
      await updateSetting("polish_model", modelId);
      if (settings?.cloud_polish_enabled) await updateSetting("cloud_polish_enabled", false);
      setSelectedPolishModel(modelId);
      analytics.track(AnalyticsEvents.SETTING_CHANGED, { setting: "polish_model", value: modelId });
      await refreshPolishModelStatus();
    } catch (error) {
      logger.error("failed_to_select_polish_model", { error: String(error) });
      showErrorToast(String(error));
    }
  };

  useEffect(() => {
    if (!settings) return;

    void refreshPolishModelStatus();
  }, [
    settings,
    selectedPolishModel,
    polishModels,
    localRuntime.provider_type,
    localRuntime.base_url,
    localRuntime.server_command,
    localRuntime.server_args_json,
  ]);

  const handleRuntimeCheck = async () => {
    setCheckingRuntime(true);
    try {
      const result = await settingsCommands.checkLocalPolishRuntimeConfig();
      setRuntimeCheckResult(result);
      await refreshPolishModelStatus();
    } catch (err) {
      logger.error("failed_to_check_local_polish_runtime", { error: String(err) });
      setRuntimeCheckResult({
        ok: false,
        kind: "provider_error",
        message: String(err),
        duration_ms: 0,
      });
    } finally {
      setCheckingRuntime(false);
    }
  };

  if (!settings) return null;

  const selectedPolishModelInfo = polishModels.find((m) => m.id === selectedPolishModel);
  const selectedCompatibilityWarning = getCompatibilityWarning(selectedPolishModelInfo, t);
  const selectedLatencyCaution = selectedPolishModelInfo
    ? getLatencyCaution(selectedPolishModelInfo, t)
    : null;
  const selectedModelStatusMatches =
    polishModelStatus?.current_model === selectedPolishModel;
  const selectedRuntimeReady = selectedModelStatusMatches
    ? polishModelStatus.runtime_ready
    : false;
  const selectedModelDownloaded = selectedModelStatusMatches
    ? polishModelStatus.is_downloaded
    : Boolean(selectedPolishModelInfo?.downloaded);
  const selectedModelLoaded = selectedModelStatusMatches
    ? polishModelStatus.is_loaded
    : false;
  const showRuntimeNotReadyWarning =
    Boolean(selectedPolishModelInfo) && selectedModelDownloaded && !selectedRuntimeReady;
  const runtimeProviderLabel = getLocalRuntimeProviderLabel(localRuntime.provider_type, t);
  const runtimeStatusText = selectedModelLoaded
    ? t("model.polish.localRuntime.readyStatus")
    : showRuntimeNotReadyWarning
      ? t("model.polish.localRuntime.notReadyStatus")
      : t("model.polish.localRuntime.fileOnlyStatus");
  const runtimeCheckMessage = getRuntimeCheckMessage(runtimeCheckResult, checkingRuntime, t);

  const serviceEnabled = settings.cloud_polish_enabled;
  const serviceProvider = settings.active_cloud_polish_provider;
  const serviceModel = settings.cloud_polish_configs[serviceProvider]?.model;
  const activeName = serviceEnabled
    ? `${serviceProvider} · ${serviceModel || t("model.catalog.configuredService")}`
    : selectedPolishModelInfo?.name ?? t("model.active.noModels");
  const latencyValue = { fast: 4, balanced: 3, slow: 2, heavy: 1 };

  return (
    <div className="model-picker rounded-2xl border border-border bg-card p-5 md:p-6">
      <ActiveModelSummary name={activeName} ready={serviceEnabled || selectedRuntimeReady} detail={serviceEnabled ? t("model.catalog.speechService") : selectedRuntimeReady ? t("model.info.ready") : t("model.catalog.runtimePending")}>
        {!serviceEnabled && selectedPolishModelInfo && <p className="mt-1 text-xs leading-5 text-muted-foreground">{getLatencyLabel(selectedPolishModelInfo, t)} · {getLatencySummary(selectedPolishModelInfo, t)}</p>}
      </ActiveModelSummary>
      {!serviceEnabled && selectedCompatibilityWarning && <p className="model-runtime-warning">{selectedCompatibilityWarning}</p>}
      {!serviceEnabled && selectedLatencyCaution && <p className="mb-4 text-xs leading-5 text-muted-foreground">{selectedLatencyCaution}</p>}
      {!serviceEnabled && showRuntimeNotReadyWarning && <p className="mb-4 text-xs leading-5 text-muted-foreground">{t("model.polish.localRuntime.notReadyHelp")}</p>}
      <ModelCatalog
        kind="polish"
        models={polishModels.map(model => ({
          id: model.id,
          name: model.name,
          description: model.built_in ? t("model.available.builtIn") : t("model.catalog.memoryRecommended", { memory: formatMemory(model.compatibility.recommended_memory_mb) }),
          sizeMb: model.built_in ? 0 : parseDisplaySize(model.size),
          sizeLabel: model.built_in ? t("model.available.builtIn") : parseDisplaySize(model.size) === null ? model.size : displaySize(parseDisplaySize(model.size) ?? 0, i18n.language),
          available: model.downloaded,
          builtIn: model.built_in,
          selected: !serviceEnabled && model.id === selectedPolishModel,
          speed: { value: latencyValue[model.latency_profile.class], maximum: 4, label: getLatencyLabel(model, t) },
          usage: getTemplateNames(model.latency_profile.recommended_templates, t),
          templates: model.latency_profile.recommended_templates,
          warning: model.built_in && !model.downloaded ? t("model.polish.appleIntelligenceHint") : getCompatibilityWarning(model, t),
          downloading: polishDownloadingId === model.id,
          progress: polishProgress,
          downloadDisabled: polishDownloadingId !== null,
        }))}
        onSelect={id => { void handlePolishModelSelect(id); }}
        onDownload={onDownload}
        onCancel={onCancel}
        onDelete={onDelete}
      />
      {!serviceEnabled && !selectedPolishModelInfo?.built_in && <details className="model-runtime-details">
        <summary>{t("model.polish.localRuntime.title")}</summary>
        <div className="space-y-4 p-1">
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0 space-y-1">
                <Label>{t("model.polish.localRuntime.title")}</Label>
                <p className="text-xs leading-5 text-muted-foreground">
                  {t("model.polish.localRuntime.description")}
                </p>
              </div>
              <div className="shrink-0 text-xs font-medium text-muted-foreground">
                {runtimeProviderLabel}
              </div>
            </div>

            <div className="grid gap-3 border-t border-border/60 pt-3 sm:grid-cols-2">
              <div className="min-w-0 space-y-1">
                <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  {t("model.polish.localRuntime.provider")}
                </p>
                <p className="truncate text-sm text-foreground">{runtimeProviderLabel}</p>
              </div>
              <div className="min-w-0 space-y-1">
                <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  {t("model.polish.localRuntime.baseUrl")}
                </p>
                <p className="truncate font-mono text-xs text-foreground/80">
                  {localRuntime.base_url}
                </p>
              </div>
            </div>

            <div className="flex items-start justify-between gap-3 border-t border-border/70 pt-3">
              <div className="min-w-0 space-y-1">
                <p className="text-sm font-medium text-foreground">{runtimeStatusText}</p>
                {runtimeCheckMessage && (
                  <p className="text-xs leading-5 text-muted-foreground">
                    {runtimeCheckMessage}
                  </p>
                )}
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-8 shrink-0 gap-1.5 px-3 text-xs"
                onClick={handleRuntimeCheck}
                disabled={checkingRuntime}
              >
                {checkingRuntime ? (
                  <CircleNotch className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Check className="h-3.5 w-3.5" />
                )}
                {t("cloud.check.button")}
              </Button>
            </div>

          </div>
      </details>}
    </div>
  );
}
