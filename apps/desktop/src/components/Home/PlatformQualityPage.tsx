import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChartLineUp, Code, Stethoscope } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useConfirm } from "@/components/ui/confirm";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  historyCommands,
  platformQualityCommands,
  type CodeContext,
  type DiagnosticReport,
  type QualityEvent,
  type QualityEventKind,
  type QualityQuery,
  type QualitySummary,
  type SetupPreset,
} from "@/lib/tauri";
import { showErrorToast, showToast } from "@/lib/toast";
import { SettingsPageLayout } from "./SettingsPageLayout";

type PeriodFilter = "all" | "days_7" | "days_30";
type SourceFilter = "all" | "local" | "cloud";
type KindFilter = "all" | QualityEventKind;

const EMPTY_SUMMARY: QualitySummary = {
  total_transcriptions: 0,
  transcription_failures: 0,
  injection_failures: 0,
  corrections: 0,
  correction_rate_percent: null,
  local_transcriptions: 0,
  cloud_transcriptions: 0,
  stt_latency_ms: { p50: null, p95: null },
  polish_latency_ms: { p50: null, p95: null },
  total_latency_ms: { p50: null, p95: null },
  application_injection_failures: {},
};

function valueOrDash(value: number | null, suffix = "") {
  return value === null ? "—" : `${Math.round(value)}${suffix}`;
}

export function PlatformQualityPage() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [diagnostics, setDiagnostics] = useState<DiagnosticReport | null>(null);
  const [diagnosticsBusy, setDiagnosticsBusy] = useState(false);
  const [latencyBusy, setLatencyBusy] = useState(false);
  const [presetBusy, setPresetBusy] = useState<SetupPreset | null>(null);
  const [context, setContext] = useState<CodeContext>({});
  const [contextBusy, setContextBusy] = useState(false);
  const [period, setPeriod] = useState<PeriodFilter>("days_30");
  const [source, setSource] = useState<SourceFilter>("all");
  const [kind, setKind] = useState<KindFilter>("all");
  const [applicationId, setApplicationId] = useState("");
  const [summary, setSummary] = useState<QualitySummary>(EMPTY_SUMMARY);
  const [events, setEvents] = useState<QualityEvent[]>([]);
  const [qualityBusy, setQualityBusy] = useState(false);
  const [exportPath, setExportPath] = useState("");
  const presets: { value: SetupPreset; label: string }[] = [
    { value: "local_only", label: t("platformQuality.presets.localOnly") },
    { value: "cloud_stt", label: t("platformQuality.presets.cloudStt") },
  ];
  const periodOptions = [
    { value: "all", label: t("platformQuality.period.all") },
    { value: "days_7", label: t("platformQuality.period.days_7") },
    { value: "days_30", label: t("platformQuality.period.days_30") },
  ];
  const sourceOptions = [
    { value: "all", label: t("platformQuality.source.all") },
    { value: "local", label: t("platformQuality.source.local") },
    { value: "cloud", label: t("platformQuality.source.cloud") },
  ];
  const kindOptions = [
    { value: "all", label: t("platformQuality.kind.all") },
    { value: "transcription_success", label: t("platformQuality.kind.transcription_success") },
    { value: "transcription_failure", label: t("platformQuality.kind.transcription_failure") },
    { value: "injection_failure", label: t("platformQuality.kind.injection_failure") },
    { value: "correction", label: t("platformQuality.kind.correction") },
  ];

  const query = useMemo<QualityQuery>(() => {
    const dayMs = 24 * 60 * 60 * 1_000;
    const sinceMs = period === "all"
      ? null
      : Date.now() - (period === "days_7" ? 7 : 30) * dayMs;
    return {
      since_ms: sinceMs,
      until_ms: null,
      application_id: applicationId.trim() || null,
      kind: kind === "all" ? null : kind,
      is_cloud: source === "all" ? null : source === "cloud",
    };
  }, [applicationId, kind, period, source]);

  const refreshQuality = useCallback(async () => {
    setQualityBusy(true);
    try {
      const [nextSummary, nextEvents] = await Promise.all([
        platformQualityCommands.getSummary(query),
        platformQualityCommands.getEvents(query),
      ]);
      setSummary(nextSummary);
      setEvents(nextEvents);
    } catch (error) {
      showErrorToast(`${t("platformQuality.error.load")} ${String(error)}`);
    } finally {
      setQualityBusy(false);
    }
  }, [query, t]);

  useEffect(() => {
    void refreshQuality();
  }, [refreshQuality]);

  useEffect(() => {
    platformQualityCommands.getCodeContext()
      .then((activeContext) => setContext(activeContext ?? {}))
      .catch((error: unknown) => showErrorToast(`${t("platformQuality.error.context")} ${String(error)}`));
  }, [t]);

  const runDiagnostics = async () => {
    setDiagnosticsBusy(true);
    try {
      setDiagnostics(await platformQualityCommands.runDiagnostics());
    } catch (error) {
      showErrorToast(`${t("platformQuality.error.diagnostics")} ${String(error)}`);
    } finally {
      setDiagnosticsBusy(false);
    }
  };

  const runLatency = async () => {
    setLatencyBusy(true);
    try {
      const mediaPath = await historyCommands.selectMediaFile();
      if (!mediaPath) return;
      const latency = await platformQualityCommands.runLatencyTest(mediaPath);
      setDiagnostics((current) => current ? { ...current, latency } : current);
      showToast(t("platformQuality.latency.completed"));
    } catch (error) {
      showErrorToast(`${t("platformQuality.error.latency")} ${String(error)}`);
    } finally {
      setLatencyBusy(false);
    }
  };

  const applyPreset = async (preset: SetupPreset) => {
    setPresetBusy(preset);
    try {
      await platformQualityCommands.applyPreset(preset);
      showToast(t("platformQuality.presets.applied"));
    } catch (error) {
      showErrorToast(`${t("platformQuality.error.preset")} ${String(error)}`);
    } finally {
      setPresetBusy(null);
    }
  };

  const saveContext = async () => {
    setContextBusy(true);
    try {
      setContext(await platformQualityCommands.setCodeContext(context));
      showToast(t("platformQuality.code.saved"));
    } catch (error) {
      showErrorToast(`${t("platformQuality.error.context")} ${String(error)}`);
    } finally {
      setContextBusy(false);
    }
  };

  const clearContext = async () => {
    setContextBusy(true);
    try {
      await platformQualityCommands.clearCodeContext();
      setContext({});
      showToast(t("platformQuality.code.cleared"));
    } catch (error) {
      showErrorToast(`${t("platformQuality.error.context")} ${String(error)}`);
    } finally {
      setContextBusy(false);
    }
  };

  const clearMetrics = async () => {
    const accepted = await confirm({
      title: t("platformQuality.clear.title"),
      description: t("platformQuality.clear.description"),
      confirmText: t("platformQuality.clear.confirm"),
      cancelText: t("common.cancel"),
      variant: "danger",
    });
    if (!accepted) return;
    try {
      await platformQualityCommands.clearMetrics();
      await refreshQuality();
      showToast(t("platformQuality.clear.completed"));
    } catch (error) {
      showErrorToast(`${t("platformQuality.error.clear")} ${String(error)}`);
    }
  };

  const exportMetrics = async () => {
    if (!exportPath.trim()) {
      showErrorToast(t("platformQuality.export.pathRequired"));
      return;
    }
    try {
      await platformQualityCommands.exportMetrics(exportPath.trim(), query, false);
      showToast(t("platformQuality.export.completed"));
    } catch (error) {
      if (String(error).includes("already exists")) {
        const accepted = await confirm({
          title: t("platformQuality.export.overwriteTitle"),
          description: t("platformQuality.export.overwriteDescription"),
          confirmText: t("platformQuality.export.overwrite"),
          cancelText: t("common.cancel"),
          variant: "danger",
        });
        if (!accepted) return;
        try {
          await platformQualityCommands.exportMetrics(exportPath.trim(), query, true);
          showToast(t("platformQuality.export.completed"));
          return;
        } catch (overwriteError) {
          showErrorToast(`${t("platformQuality.error.export")} ${String(overwriteError)}`);
          return;
        }
      }
      showErrorToast(`${t("platformQuality.error.export")} ${String(error)}`);
    }
  };

  return (
    <SettingsPageLayout
      title={t("platformQuality.title")}
      description={t("platformQuality.description")}
      testId="platform-quality-page"
    >
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Stethoscope />{t("platformQuality.diagnostics.title")}</CardTitle>
          <CardDescription>{t("platformQuality.diagnostics.description")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap gap-2">
            <Button onClick={runDiagnostics} disabled={diagnosticsBusy}>
              {diagnosticsBusy ? t("platformQuality.diagnostics.running") : t("platformQuality.diagnostics.run")}
            </Button>
            <Button variant="outline" onClick={runLatency} disabled={latencyBusy}>
              {latencyBusy ? t("platformQuality.latency.running") : t("platformQuality.latency.run")}
            </Button>
          </div>
          {diagnostics && (
            <div className="grid gap-3 md:grid-cols-3" data-testid="diagnostic-report">
              <Metric label={t("platformQuality.diagnostics.microphone")} value={diagnostics.microphone.ready ? t("platformQuality.ready") : t("platformQuality.notReady")} />
              <Metric label={t("platformQuality.diagnostics.hardware")} value={`${diagnostics.hardware.logical_cpu_count} ${t("platformQuality.diagnostics.cpus")} · ${valueOrDash(diagnostics.hardware.total_memory_mb, " MB")}`} />
              <Metric label={t("platformQuality.diagnostics.model")} value={diagnostics.recommended_model.model_name} />
              <Metric
                label={t("platformQuality.diagnostics.recommendation")}
                value={diagnosticRecommendation(diagnostics, t)}
              />
              <Metric label={t("platformQuality.latency.stt")} value={valueOrDash(diagnostics.latency?.stt_ms ?? null, " ms")} />
              <Metric label={t("platformQuality.latency.total")} value={valueOrDash(diagnostics.latency?.total_ms ?? null, " ms")} />
            </div>
          )}
          <div className="grid gap-3 md:grid-cols-3">
            {presets.map((preset) => (
              <Button key={preset.value} variant="outline" onClick={() => applyPreset(preset.value)} disabled={presetBusy !== null}>
                {preset.label}
              </Button>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Code />{t("platformQuality.code.title")}</CardTitle>
          <CardDescription>{t("platformQuality.code.description")}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-2">
          <ContextField id="code-language" label={t("platformQuality.code.language")} value={context.language ?? ""} onChange={(language) => setContext((current) => ({ ...current, language }))} />
          <ContextField id="code-editor" label={t("platformQuality.code.editor")} value={context.editor_id ?? ""} onChange={(editor_id) => setContext((current) => ({ ...current, editor_id }))} />
          <ContextField id="code-file" label={t("platformQuality.code.file")} value={context.file_path ?? ""} onChange={(file_path) => setContext((current) => ({ ...current, file_path }))} />
          <ContextField id="code-symbol" label={t("platformQuality.code.symbol")} value={context.symbol ?? ""} onChange={(symbol) => setContext((current) => ({ ...current, symbol }))} />
          <div className="flex gap-2 md:col-span-2">
            <Button onClick={saveContext} disabled={contextBusy}>{t("common.save")}</Button>
            <Button variant="outline" onClick={clearContext} disabled={contextBusy}>{t("platformQuality.code.clear")}</Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><ChartLineUp />{t("platformQuality.quality.title")}</CardTitle>
          <CardDescription>{t("platformQuality.quality.description")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="grid gap-3 md:grid-cols-4">
            <FilterSelect id="filter-period" label={t("platformQuality.filters.period")} value={period} onChange={(value) => setPeriod(value as PeriodFilter)} options={periodOptions} />
            <FilterSelect id="filter-source" label={t("platformQuality.filters.source")} value={source} onChange={(value) => setSource(value as SourceFilter)} options={sourceOptions} />
            <FilterSelect id="filter-outcome" label={t("platformQuality.filters.outcome")} value={kind} onChange={(value) => setKind(value as KindFilter)} options={kindOptions} />
            <ContextField id="quality-application" label={t("platformQuality.filters.application")} value={applicationId} onChange={setApplicationId} />
          </div>

          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4" aria-busy={qualityBusy}>
            <Metric label={t("platformQuality.summary.successes")} value={String(summary.total_transcriptions)} />
            <Metric label={t("platformQuality.summary.failures")} value={String(summary.transcription_failures)} />
            <Metric label={t("platformQuality.summary.injectionFailures")} value={String(summary.injection_failures)} />
            <Metric label={t("platformQuality.summary.correctionRate")} value={valueOrDash(summary.correction_rate_percent, "%")} />
            <Metric label={t("platformQuality.summary.localCloud")} value={`${summary.local_transcriptions} / ${summary.cloud_transcriptions}`} />
            <Metric label={t("platformQuality.summary.sttLatency")} value={`${valueOrDash(summary.stt_latency_ms.p50, " ms")} / ${valueOrDash(summary.stt_latency_ms.p95, " ms")}`} />
            <Metric label={t("platformQuality.summary.polishLatency")} value={`${valueOrDash(summary.polish_latency_ms.p50, " ms")} / ${valueOrDash(summary.polish_latency_ms.p95, " ms")}`} />
            <Metric label={t("platformQuality.summary.totalLatency")} value={`${valueOrDash(summary.total_latency_ms.p50, " ms")} / ${valueOrDash(summary.total_latency_ms.p95, " ms")}`} />
          </div>

          <div data-testid="quality-events" className="rounded-2xl border border-border/70 divide-y divide-border/70">
            {events.length === 0 ? (
              <p className="p-4 text-sm text-muted-foreground">{t("platformQuality.events.empty")}</p>
            ) : events.slice(-20).reverse().map((event) => (
              <div key={`${event.created_at_ms}-${event.kind}-${event.application_id ?? ""}`} className="grid grid-cols-4 gap-3 p-3 text-sm">
                <span>{qualityKindLabel(event.kind, t)}</span>
                <span>{event.application_id ?? t("platformQuality.events.unknownApplication")}</span>
                <span>{event.is_cloud === null ? "—" : event.is_cloud ? t("platformQuality.source.cloud") : t("platformQuality.source.local")}</span>
                <span className="text-right">{valueOrDash(event.total_ms, " ms")}</span>
              </div>
            ))}
          </div>

          <div data-testid="application-injection-failures" className="rounded-2xl border border-border/70 p-4">
            <p className="text-sm font-medium">{t("platformQuality.appFailures.title")}</p>
            {Object.keys(summary.application_injection_failures).length === 0 ? (
              <p className="mt-2 text-sm text-muted-foreground">{t("platformQuality.appFailures.empty")}</p>
            ) : (
              <dl className="mt-3 space-y-2">
                {Object.entries(summary.application_injection_failures).map(([appId, count]) => (
                  <div key={appId} className="flex items-center justify-between gap-4 text-sm">
                    <dt className="truncate">{appId}</dt>
                    <dd className="font-medium">{count}</dd>
                  </div>
                ))}
              </dl>
            )}
          </div>

          <div className="flex flex-col gap-3 md:flex-row md:items-end">
            <ContextField id="quality-export-path" label={t("platformQuality.export.path")} value={exportPath} onChange={setExportPath} />
            <Button variant="outline" onClick={exportMetrics}>{t("platformQuality.export.button")}</Button>
            <Button variant="ghost" className="text-destructive" onClick={clearMetrics}>{t("platformQuality.clear.button")}</Button>
          </div>
        </CardContent>
      </Card>
    </SettingsPageLayout>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return <div className="rounded-2xl bg-muted/45 p-4"><p className="text-xs text-muted-foreground">{label}</p><p className="mt-1 font-medium break-words">{value}</p></div>;
}

function ContextField({ id, label, value, onChange }: { id: string; label: string; value: string; onChange: (value: string) => void }) {
  return <div className="space-y-2 min-w-0"><Label htmlFor={id}>{label}</Label><Input id={id} value={value} onChange={(event) => onChange(event.target.value)} /></div>;
}

function FilterSelect({ id, label, value, onChange, options }: { id: string; label: string; value: string; onChange: (value: string) => void; options: { value: string; label: string }[] }) {
  return <div className="space-y-2"><Label htmlFor={id}>{label}</Label><select id={id} className="h-10 w-full rounded-2xl border border-input bg-background px-3 text-sm" value={value} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></div>;
}

function qualityKindLabel(kind: QualityEventKind, t: (key: string) => string) {
  switch (kind) {
    case "transcription_success": return t("platformQuality.kind.transcription_success");
    case "transcription_failure": return t("platformQuality.kind.transcription_failure");
    case "injection_failure": return t("platformQuality.kind.injection_failure");
    case "correction": return t("platformQuality.kind.correction");
  }
}

function diagnosticRecommendation(
  report: DiagnosticReport,
  t: (key: string, options?: { model: string }) => string,
) {
  return report.recommended_preset === null
    ? t("platformQuality.diagnostics.microphoneRequired")
    : t("platformQuality.diagnostics.localReason", { model: report.recommended_model.model_name });
}
