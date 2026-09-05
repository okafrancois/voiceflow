import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useSettingsContext } from "@/contexts/SettingsContext";
import { useEventListeners } from "@/hooks/useEventListeners";
import { homeCommands, historyCommands, events, type DictationHome, type TranscriptionEntry } from "@/lib/tauri";
import { useUsageStatistics } from "@/hooks/useUsageStatistics";
import { UsageSummary } from "./UsageSummary";
import { HistoryEntryCard } from "./HistoryPage";
import { SettingsPageLayout } from "./SettingsPageLayout";

export function Dashboard() {
  const { t } = useTranslation();
  const { settings } = useSettingsContext();
  const [snapshot, setSnapshot] = useState<DictationHome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [entries, setEntries] = useState<TranscriptionEntry[]>([]);
  const [historyLoading, setHistoryLoading] = useState(true);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const usage = useUsageStatistics("7d");
  const historyRequestId = useRef(0);
  const refreshHistory = useCallback(async () => {
    const id = ++historyRequestId.current;
    setHistoryLoading(true);
    try {
      const result = await historyCommands.getHistory({ limit: 5, offset: 0 });
      if (id === historyRequestId.current) { setEntries(result); setHistoryError(null); }
    } catch (caught) {
      if (id === historyRequestId.current) setHistoryError(String(caught));
    } finally {
      if (id === historyRequestId.current) setHistoryLoading(false);
    }
  }, []);
  useEffect(() => {
    void refreshHistory();
    window.addEventListener("focus", refreshHistory);
    return () => { ++historyRequestId.current; window.removeEventListener("focus", refreshHistory); };
  }, [refreshHistory]);
  useEventListeners(async () => Promise.all([
    events.onTranscriptionComplete(refreshHistory), events.onTranscriptionError(refreshHistory),
  ]), [refreshHistory]);
  const onEntryChanged = async () => { await Promise.all([refreshHistory(), usage.refresh()]); };
  const requestId = useRef(0);
  const refresh = useCallback(async () => {
    const id = ++requestId.current;
    try {
      const next = await homeCommands.getSnapshot();
      if (id === requestId.current) { setSnapshot(next); setError(null); }
    } catch (caught) {
      if (id === requestId.current) setError(String(caught));
    }
  }, []);
  useEffect(() => {
    void refresh();
    window.addEventListener("focus", refresh);
    return () => { ++requestId.current; window.removeEventListener("focus", refresh); };
  }, [refresh, settings]);
  useEventListeners(async () => Promise.all([
    events.onTranscriptionComplete(refresh), events.onTranscriptionError(refresh),
    events.onModelDownloadComplete(refresh), events.onModelDeleted(refresh),
  ]), [refresh]);

  const readinessLabels: Record<DictationHome["readiness"], string> = {
    ready: t("home.readiness.ready"),
    microphone_required: t("home.readiness.microphone_required"),
    permissions_required: t("home.readiness.permissions_required"),
    model_required: t("home.readiness.model_required"),
    cloud_configuration_required: t("home.readiness.cloud_configuration_required"),
  };
  const instructions: Record<DictationHome["trigger_mode"], string> = {
    hold: t("home.instructions.hold", { hotkey: snapshot?.hotkey }),
    toggle: t("home.instructions.toggle", { hotkey: snapshot?.hotkey }),
    double_tap: t("home.instructions.double_tap", { hotkey: snapshot?.hotkey }),
  };

  return (
    <SettingsPageLayout title={t("home.title")} description={t("home.description")} testId="dictation-home">
      {error && <div role="alert" className="rounded-2xl border border-destructive/40 p-4 text-sm"><p>{error}</p><Button variant="outline" onClick={refresh}>{t("common.retry")}</Button></div>}
      {!snapshot && !error && <p role="status">{t("common.loading")}</p>}
      {snapshot && <div className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-border/60 bg-secondary/30 p-4">
        <div className="space-y-1">
          {snapshot.readiness !== "ready" && <p className="text-sm font-medium">{readinessLabels[snapshot.readiness]}</p>}
          <p className="text-xs text-muted-foreground">{instructions[snapshot.trigger_mode]}</p>
          {snapshot.setup_path && <Link className="text-xs underline underline-offset-4" to={snapshot.setup_path}>{t("home.openSetup")}</Link>}
        </div>
        <Link to="/hotkey" aria-label={t("home.changeShortcut")} className="flex items-center gap-3 text-xs text-muted-foreground">
          {t(snapshot.is_cloud ? "home.cloudTranscription" : "home.localTranscription")}
          <kbd className="rounded-2xl border border-border bg-background px-3 py-2 font-mono text-sm">{snapshot.hotkey}</kbd>
        </Link>
      </div>}
      <section className="space-y-3" aria-label={t("usage.period7")}>
        <div className="flex items-center justify-between gap-3"><h2 className="text-sm font-medium">{t("usage.period7")}</h2><Link className="text-xs underline underline-offset-4" to="/statistics">{t("usage.openStatistics")}</Link></div>
        {usage.statistics && <UsageSummary statistics={usage.statistics} />}
        {usage.loading && !usage.statistics && <p role="status" className="text-sm text-muted-foreground">{t("common.loading")}</p>}
        {usage.error && <div role="alert" className="text-sm text-destructive">{usage.error}<Button variant="ghost" onClick={usage.refresh}>{t("common.retry")}</Button></div>}
      </section>
      <Card><CardHeader className="flex-row items-center justify-between gap-3"><CardTitle>{t("usage.recent")}</CardTitle><Link className="text-xs underline underline-offset-4" to="/history">{t("home.openHistory")}</Link></CardHeader>
        <CardContent className="px-2 md:px-2">
          {historyError ? <div role="alert" className="p-4 text-sm text-destructive">{historyError}<Button variant="ghost" onClick={refreshHistory}>{t("common.retry")}</Button></div>
            : historyLoading && !entries.length ? <p role="status" className="p-5 text-sm text-muted-foreground">{t("common.loading")}</p>
            : entries.length ? entries.map(entry => <HistoryEntryCard key={entry.id} entry={entry} advanced={false} t={t} onChanged={onEntryChanged} />)
            : <p className="p-5 text-sm leading-7 text-muted-foreground">{t("home.noResult")}</p>}
        </CardContent>
      </Card>
    </SettingsPageLayout>
  );
}
