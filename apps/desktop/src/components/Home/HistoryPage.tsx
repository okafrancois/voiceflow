import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useTranslation } from "react-i18next";
import {
  ArrowCounterClockwise, CaretDown, CaretLeft, CaretRight, CaretUp, Clock,
  CopySimple, Export, FileAudio, MagnifyingGlass, MagicWand, PaperPlaneTilt,
  Play, Trash, Translate, UploadSimple, WarningCircle, X,
} from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { useConfirm } from "@/components/ui/confirm";
import { Input } from "@/components/ui/input";
import { SegmentedControl } from "@/components/ui/segmented-control";
import {
  ExportFormat, FileTranscriptionJob, HistoryFilter, HistoryTextVersion, TranscriptionEntry,
  events, historyCommands,
} from "@/lib/tauri";
import { logger } from "@/lib/logger";
import { showErrorToast, showInfoToast, showToast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { supportedLanguages } from "@/i18n";
import { SettingsPageLayout } from "./SettingsPageLayout";

const PAGE_SIZE = 20;
const SUPPORTED_MEDIA = /\.(wav|mp3|m4a|flac|ogg|mp4|mov|webm)$/i;

type EngineFilter = "all" | "local" | "cloud";
type ImportState = "idle" | "queued" | "running" | "completed" | "error" | "canceled";

const LANGUAGE_OPTIONS = supportedLanguages.map(({ code, name }) => ({ value: code, label: name }));

function importStateLabel(
  state: Exclude<ImportState, "idle">,
  t: (key: string) => string,
): string {
  switch (state) {
    case "queued": return t("history.workbench.queued");
    case "running": return t("history.workbench.running");
    case "completed": return t("history.workbench.completed");
    case "error": return t("history.workbench.error");
    case "canceled": return t("history.workbench.canceled");
  }
}

function deliveryStatusLabel(status: string, t: (key: string) => string): string {
  switch (status) {
    case "pending_insertion": return t("history.delivery.pending");
    case "inserted_keyboard": return t("history.delivery.insertedKeyboard");
    case "inserted_clipboard": return t("history.delivery.insertedClipboard");
    case "inserted_accessibility": return t("history.delivery.insertedAccessibility");
    case "inserted_stream": return t("history.delivery.insertedStream");
    case "copied": return t("history.delivery.copied");
    case "copy_failed": return t("history.delivery.copyFailed");
    case "failed": return t("history.delivery.failed");
    case "not_recorded":
    case "not_delivered": return t("history.delivery.notDelivered");
    default: return t("history.delivery.unknown");
  }
}

function formatRelativeTime(
  timestamp: number,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  const minutes = Math.floor((Date.now() - timestamp) / 60_000);
  if (minutes < 1) return t("history.justNow");
  if (minutes < 60) return t("history.minutesAgo", { captures: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("history.hoursAgo", { captures: hours });
  const days = Math.floor(hours / 24);
  if (days < 7) return t("history.daysAgo", { captures: days });
  return new Intl.DateTimeFormat().format(new Date(timestamp));
}

function formatDuration(milliseconds: number | null): string {
  if (milliseconds === null) return "—";
  if (milliseconds < 1_000) return `${milliseconds} ms`;
  return `${(milliseconds / 1_000).toFixed(1)} s`;
}

interface ImportWorkbenchProps {
  onCompleted: () => Promise<void>;
  t: (key: string, options?: Record<string, unknown>) => string;
}

function ImportWorkbench({ onCompleted, t }: ImportWorkbenchProps) {
  const [translationTarget, setTranslationTarget] = useState("");
  const [state, setState] = useState<ImportState>("idle");
  const [path, setPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [job, setJob] = useState<FileTranscriptionJob | null>(null);
  const [jobs, setJobs] = useState<FileTranscriptionJob[]>([]);
  const activeJobId = useRef<string | null>(null);
  const handledJobs = useRef(new Set<string>());

  const applyJobUpdate = useCallback((updated: FileTranscriptionJob) => {
    setJobs((current) => {
      const existing = current.some((candidate) => candidate.id === updated.id);
      return existing
        ? current.map((candidate) => candidate.id === updated.id ? updated : candidate)
        : [...current, updated];
    });
    if (activeJobId.current !== updated.id) return;
    setJob(updated);
    setState(updated.state);
    setError(updated.error);
    if (updated.state === "completed" && !handledJobs.current.has(updated.id)) {
      handledJobs.current.add(updated.id);
      void onCompleted();
      showToast(t("history.workbench.completed"));
    }
    if (updated.state === "error" && updated.error) showErrorToast(updated.error);
  }, [onCompleted, t]);

  const transcribe = useCallback(async (mediaPath: string) => {
    if (!SUPPORTED_MEDIA.test(mediaPath)) {
      const message = t("history.workbench.unsupported");
      setError(message);
      setState("error");
      showErrorToast(message);
      return;
    }
    setPath(mediaPath);
    setState("queued");
    setError(null);
    try {
      const started = await historyCommands.startFileJob({
        path: mediaPath,
        profile_id: null,
        translation_target: translationTarget || null,
      });
      activeJobId.current = started.id;
      applyJobUpdate(started);
      // The backend can advance before the start command resolves. Reconcile once
      // so a fast completion cannot be lost between the command and event listener.
      applyJobUpdate(await historyCommands.getFileJob(started.id));
    } catch (caught) {
      const message = String(caught);
      setError(message);
      setState("error");
      logger.error("media_import_failed", { path: mediaPath, error: message });
      showErrorToast(message);
    }
  }, [applyJobUpdate, t, translationTarget]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void events.onFileTranscriptionJobChanged(applyJobUpdate)
      .then((stop) => { unlisten = stop; });
    return () => unlisten?.();
  }, [applyJobUpdate]);

  useEffect(() => {
    void historyCommands.listFileJobs().then(setJobs).catch((caught) => {
      logger.error("media_job_list_failed", { error: String(caught) });
    });
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void getCurrentWebviewWindow().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        setIsDragging(true);
        return;
      }
      if (event.payload.type === "leave") {
        setIsDragging(false);
        return;
      }
      setIsDragging(false);
      const dropped = event.payload.paths[0];
      if (dropped) void transcribe(dropped);
    }).then((stop) => { unlisten = stop; }).catch((caught) => {
      logger.error("media_drop_listener_failed", { error: String(caught) });
    });
    return () => unlisten?.();
  }, [transcribe]);

  const chooseFile = async () => {
    try {
      const selected = await historyCommands.selectMediaFile();
      if (selected) await transcribe(selected);
    } catch (caught) {
      showErrorToast(String(caught));
    }
  };

  const cancelJob = async () => {
    if (!job || (job.state !== "queued" && job.state !== "running")) return;
    try {
      const canceled = await historyCommands.cancelFileJob(job.id);
      applyJobUpdate(canceled);
    } catch (caught) {
      showErrorToast(String(caught));
    }
  };

  const cancelListedJob = async (id: string) => {
    try {
      applyJobUpdate(await historyCommands.cancelFileJob(id));
    } catch (caught) {
      showErrorToast(String(caught));
    }
  };

  return (
    <Card className={cn("border-dashed p-5 transition-colors", isDragging && "border-primary bg-primary/5")} data-testid="media-import-workbench">
      <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div className="flex min-w-0 items-start gap-3">
          <div className="rounded-2xl bg-secondary p-3"><UploadSimple className="h-5 w-5" /></div>
          <div className="min-w-0">
            <h2 className="font-semibold">{t("history.workbench.title")}</h2>
            <p className="text-sm text-muted-foreground">{t("history.workbench.description")}</p>
            {path && <p className="mt-1 truncate text-xs text-muted-foreground">{path}</p>}
          </div>
        </div>
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
          <label className="text-xs text-muted-foreground" htmlFor="import-translation-target">{t("history.translation.target")}</label>
          <select
            id="import-translation-target"
            value={translationTarget}
            onChange={(event) => setTranslationTarget(event.target.value)}
            disabled={state === "running"}
            className="h-9 rounded-xl border border-border bg-background px-3 text-sm"
          >
            <option value="">{t("history.translation.none")}</option>
            {LANGUAGE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <Button onClick={chooseFile} disabled={state === "running"}>
            <FileAudio className="mr-2 h-4 w-4" />{t("history.workbench.choose")}
          </Button>
        </div>
      </div>
      {state !== "idle" && (
        <div className="mt-4 flex items-center gap-2 text-sm" aria-live="polite">
          {state === "running" && <ArrowCounterClockwise className="h-4 w-4 animate-spin" />}
          <span>{importStateLabel(state, t)}</span>
          {job && (state === "queued" || state === "running") && (
            <>
              <span>{job.progress_percent}%</span>
              <Button variant="ghost" size="sm" onClick={cancelJob}>{t("history.workbench.cancel")}</Button>
            </>
          )}
          {error && <span className="text-destructive">{error}</span>}
        </div>
      )}
      {jobs.length > 0 && (
        <div className="mt-4 space-y-2" data-testid="media-job-list">
          {jobs.map((listedJob) => (
            <div key={listedJob.id} className="flex items-center gap-2 rounded-xl bg-secondary/50 px-3 py-2 text-xs">
              <span className="min-w-0 flex-1 truncate">{listedJob.request.path}</span>
              <span>{importStateLabel(listedJob.state, t)}</span>
              <span>{listedJob.progress_percent}%</span>
              {(listedJob.state === "queued" || listedJob.state === "running") && listedJob.id !== job?.id && (
                <Button variant="ghost" size="sm" onClick={() => cancelListedJob(listedJob.id)}>{t("history.workbench.cancel")}</Button>
              )}
            </div>
          ))}
        </div>
      )}
    </Card>
  );
}

interface HistoryEntryCardProps {
  entry: TranscriptionEntry;
  onChanged: (entry?: TranscriptionEntry) => Promise<void>;
  t: (key: string, options?: Record<string, unknown>) => string;
}

function HistoryEntryCard({ entry, onChanged, t }: HistoryEntryCardProps) {
  const confirm = useConfirm();
  const [expanded, setExpanded] = useState(false);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [translationTarget, setTranslationTarget] = useState(entry.translation_target ?? "");
  const [audioUrl, setAudioUrl] = useState<string | null>(null);

  useEffect(() => () => { if (audioUrl) URL.revokeObjectURL(audioUrl); }, [audioUrl]);

  const run = async (name: string, action: () => Promise<void>) => {
    setBusyAction(name);
    try {
      await action();
    } catch (caught) {
      logger.error("history_action_failed", { id: entry.id, action: name, error: String(caught) });
      showErrorToast(String(caught));
    } finally {
      setBusyAction(null);
    }
  };

  const copy = (version: HistoryTextVersion) => run(`copy-${version}`, async () => {
    await historyCommands.copyEntry(entry.id, version);
    showToast(t("history.copied"));
    await onChanged();
  });

  const reinsert = (version: HistoryTextVersion) => run(`insert-${version}`, async () => {
    await historyCommands.reinsertEntry(entry.id, version);
    showToast(t("history.actions.inserted"));
    await onChanged();
  });

  const repolish = (target: string | null) => run(target ? "translate" : "polish", async () => {
    const updated = await historyCommands.repolishEntry(entry.id, null, target);
    await onChanged(updated);
    showToast(target ? t("history.translation.completed") : t("history.actions.polished"));
  });

  const retranscribe = () => run("retranscribe", async () => {
    const updated = await historyCommands.retranscribeEntry(entry.id);
    await onChanged(updated);
    showToast(t("history.actions.retranscribed"));
  });

  const exportEntry = (format: ExportFormat) => run(`export-${format}`, async () => {
    const outputPath = await historyCommands.selectExportFile(format);
    if (!outputPath) return;
    try {
      await historyCommands.exportEntry(entry.id, format, outputPath, false);
    } catch (caught) {
      if (!String(caught).includes("overwrite was not confirmed")) throw caught;
      const overwrite = await confirm({
        title: t("history.export.overwriteTitle"),
        description: t("history.export.overwriteDescription"),
        confirmText: t("history.export.overwrite"),
        cancelText: t("history.cancel"),
        variant: "danger",
      });
      if (!overwrite) return;
      await historyCommands.exportEntry(entry.id, format, outputPath, true);
    }
    showToast(t("history.export.completed"));
  });

  const loadAudio = () => run("audio", async () => {
    const payload = await historyCommands.getAudio(entry.id);
    if (audioUrl) URL.revokeObjectURL(audioUrl);
    const data = new Uint8Array(payload.bytes);
    setAudioUrl(URL.createObjectURL(new Blob([data.buffer], { type: payload.mime_type })));
  });

  const deleteEntry = () => run("delete", async () => {
    const accepted = await confirm({
      title: t("history.actions.deleteTitle"),
      description: t("history.actions.deleteDescription"),
      confirmText: t("history.confirmDelete"),
      cancelText: t("history.cancel"),
      variant: "danger",
    });
    if (!accepted) return;
    await historyCommands.deleteEntry(entry.id);
    await onChanged();
    showToast(t("history.actions.deleted"));
  });

  const isError = entry.status === "error";
  const hasMedia = Boolean(entry.audio_path || entry.source_path);

  return (
    <article className="border-b border-border/50 last:border-0" data-testid={`history-entry-${entry.id}`}>
      <div className="flex items-start justify-between gap-4 p-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            {isError && <WarningCircle className="h-4 w-4 text-destructive" />}
            <span>{entry.source_kind === "file" ? t("history.source.file") : t("history.source.recording")}</span>
            <span>·</span><span>{entry.stt_engine}</span>
            {entry.language && <span>· {entry.language}</span>}
            {entry.translation_target && <span>→ {entry.translation_target}</span>}
          </div>
          <p className={cn("mt-2 line-clamp-3 text-sm", isError && "text-destructive")}>
            {isError ? entry.error ?? t("history.error.failed") : entry.final_text}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span className="text-xs text-muted-foreground">{formatRelativeTime(entry.created_at, t)}</span>
          <Button variant="ghost" size="sm" onClick={() => setExpanded((value) => !value)} aria-label={t("history.actions.details")}>
            {expanded ? <CaretUp className="h-4 w-4" /> : <CaretDown className="h-4 w-4" />}
          </Button>
        </div>
      </div>

      {expanded && (
        <div className="space-y-4 border-t border-border/40 bg-secondary/10 p-4">
          <div className="grid gap-3 md:grid-cols-2">
            <section className="rounded-2xl border border-border/50 bg-background p-3">
              <div className="mb-2 flex items-center justify-between">
                <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t("history.details.raw")}</h3>
                <div className="flex gap-1">
                  <Button variant="ghost" size="sm" onClick={() => copy("raw")} aria-label={t("history.actions.copyRaw")}><CopySimple className="h-4 w-4" /></Button>
                  <Button variant="ghost" size="sm" onClick={() => reinsert("raw")} aria-label={t("history.actions.insertRaw")}><PaperPlaneTilt className="h-4 w-4" /></Button>
                </div>
              </div>
              <p className="whitespace-pre-wrap text-sm">{entry.raw_text || "—"}</p>
            </section>
            <section className="rounded-2xl border border-border/50 bg-background p-3">
              <div className="mb-2 flex items-center justify-between">
                <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t("history.details.final")}</h3>
                <div className="flex gap-1">
                  <Button variant="ghost" size="sm" onClick={() => copy("final")} aria-label={t("history.actions.copyFinal")}><CopySimple className="h-4 w-4" /></Button>
                  <Button variant="ghost" size="sm" onClick={() => reinsert("final")} aria-label={t("history.actions.insertFinal")}><PaperPlaneTilt className="h-4 w-4" /></Button>
                </div>
              </div>
              <p className="whitespace-pre-wrap text-sm">{entry.final_text || "—"}</p>
            </section>
          </div>

          <dl className="grid grid-cols-2 gap-2 text-xs text-muted-foreground md:grid-cols-4">
            <div><dt>{t("history.details.model")}</dt><dd className="text-foreground">{entry.stt_model ?? "—"}</dd></div>
            <div><dt>{t("history.details.audioDuration")}</dt><dd className="text-foreground">{formatDuration(entry.audio_duration_ms)}</dd></div>
            <div><dt>{t("history.details.processing")}</dt><dd className="text-foreground">{formatDuration(entry.total_duration_ms)}</dd></div>
            <div><dt>{t("history.details.delivery")}</dt><dd className="text-foreground">{deliveryStatusLabel(entry.delivery_status, t)}</dd></div>
          </dl>

          {audioUrl ? <audio className="w-full" controls src={audioUrl} /> : hasMedia ? (
            <Button variant="outline" size="sm" onClick={loadAudio} disabled={busyAction === "audio"}><Play className="mr-2 h-4 w-4" />{t("history.actions.playAudio")}</Button>
          ) : null}

          <div className="flex flex-wrap items-center gap-2">
            <Button variant="outline" size="sm" onClick={retranscribe} disabled={!hasMedia || busyAction !== null}><ArrowCounterClockwise className="mr-2 h-4 w-4" />{t("history.actions.retranscribe")}</Button>
            <Button variant="outline" size="sm" onClick={() => repolish(null)} disabled={!entry.raw_text || busyAction !== null}><MagicWand className="mr-2 h-4 w-4" />{t("history.actions.repolish")}</Button>
            <label className="sr-only" htmlFor={`translation-${entry.id}`}>{t("history.translation.target")}</label>
            <select
              id={`translation-${entry.id}`}
              aria-label={t("history.translation.target")}
              value={translationTarget}
              onChange={(event) => setTranslationTarget(event.target.value)}
              className="h-9 rounded-xl border border-border bg-background px-3 text-sm"
            >
              <option value="">{t("history.translation.none")}</option>
              {LANGUAGE_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
            </select>
            <Button variant="outline" size="sm" onClick={() => repolish(translationTarget || null)} disabled={!translationTarget || busyAction !== null} aria-label={t("history.actions.translate")}><Translate className="mr-2 h-4 w-4" />{t("history.actions.translate")}</Button>
            {(["txt", "markdown", "srt"] as ExportFormat[]).map((format) => (
              <Button key={format} variant="ghost" size="sm" onClick={() => exportEntry(format)} disabled={busyAction !== null}><Export className="mr-1 h-4 w-4" />{format === "markdown" ? "MD" : format.toUpperCase()}</Button>
            ))}
            <Button variant="ghost" size="sm" onClick={deleteEntry} className="ml-auto text-destructive" disabled={busyAction !== null}><Trash className="mr-1 h-4 w-4" />{t("common.delete")}</Button>
          </div>
        </div>
      )}
    </article>
  );
}

export function HistoryPage() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [entries, setEntries] = useState<TranscriptionEntry[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [searchQuery, setSearchQuery] = useState("");
  const [pendingSearch, setPendingSearch] = useState("");
  const [engineFilter, setEngineFilter] = useState<EngineFilter>("all");
  const [currentPage, setCurrentPage] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => { setPendingSearch(searchQuery.trim()); setCurrentPage(0); }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  const fetchHistory = useCallback(async () => {
    setIsLoading(true);
    try {
      const filter: HistoryFilter = {
        limit: PAGE_SIZE,
        offset: currentPage * PAGE_SIZE,
        search: pendingSearch || undefined,
        engine: engineFilter === "all" ? undefined : engineFilter,
      };
      const [result, count] = await Promise.all([historyCommands.getHistory(filter), historyCommands.getHistoryCount(filter)]);
      setEntries(result);
      setTotalCount(count);
    } catch (caught) {
      logger.error("history_fetch_failed", { error: String(caught) });
      showErrorToast(String(caught));
    } finally {
      setIsLoading(false);
    }
  }, [currentPage, engineFilter, pendingSearch]);

  useEffect(() => { void fetchHistory(); }, [fetchHistory]);

  const onEntryChanged = useCallback(async (updated?: TranscriptionEntry) => {
    if (updated) setEntries((current) => current.map((entry) => entry.id === updated.id ? updated : entry));
    else await fetchHistory();
  }, [fetchHistory]);

  const clearHistory = async () => {
    const accepted = await confirm({ title: t("history.clearAll"), description: t("history.clearAllConfirm"), confirmText: t("history.confirmDelete"), cancelText: t("history.cancel"), variant: "danger" });
    if (!accepted) return;
    try {
      await historyCommands.clearAll();
      setEntries([]); setTotalCount(0); setCurrentPage(0);
      showInfoToast(t("history.clear.success"));
    } catch (caught) { showErrorToast(String(caught)); }
  };

  const filters = useMemo(() => [
    { value: "all" as const, label: t("history.filter.all") },
    { value: "local" as const, label: t("history.filter.local") },
    { value: "cloud" as const, label: t("history.filter.cloud") },
  ], [t]);
  const totalPages = Math.max(1, Math.ceil(totalCount / PAGE_SIZE));

  return (
    <SettingsPageLayout title={t("history.title")} description={t("history.description")} testId="history-page">
      <ImportWorkbench onCompleted={fetchHistory} t={t} />
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <SegmentedControl items={filters} value={engineFilter} onChange={(value) => { setEngineFilter(value as EngineFilter); setCurrentPage(0); }} size="sm" />
        <div className="relative w-full sm:w-72">
          <MagnifyingGlass className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input value={searchQuery} onChange={(event) => setSearchQuery(event.target.value)} placeholder={t("history.search.placeholder")} className="rounded-full pl-10 pr-10" />
          {searchQuery && <Button variant="ghost" size="icon" className="absolute right-1 top-1/2 h-7 w-7 -translate-y-1/2" onClick={() => setSearchQuery("")}><X className="h-4 w-4" /></Button>}
        </div>
      </div>

      <Card className="min-h-[320px]">
        {isLoading && entries.length === 0 ? <div className="p-10 text-center text-sm text-muted-foreground">{t("history.loading")}</div> : entries.length === 0 ? (
          <div className="flex flex-col items-center justify-center p-16 text-center"><Clock className="mb-3 h-8 w-8 text-muted-foreground" /><h2 className="font-semibold">{t("history.empty.title")}</h2><p className="mt-1 text-sm text-muted-foreground">{t("history.empty.description")}</p></div>
        ) : <div data-testid="history-entries">{entries.map((entry) => <HistoryEntryCard key={entry.id} entry={entry} onChanged={onEntryChanged} t={t} />)}</div>}
        {entries.length > 0 && (
          <div className="flex items-center justify-between border-t border-border/50 p-4">
            <span className="text-sm text-muted-foreground">{t("history.pagination.page", { current: currentPage + 1, total: totalPages })}</span>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" disabled={currentPage === 0} onClick={() => setCurrentPage((page) => page - 1)}><CaretLeft className="mr-1 h-4 w-4" />{t("history.pagination.prev")}</Button>
              <Button variant="outline" size="sm" disabled={currentPage + 1 >= totalPages} onClick={() => setCurrentPage((page) => page + 1)}>{t("history.pagination.next")}<CaretRight className="ml-1 h-4 w-4" /></Button>
            </div>
          </div>
        )}
      </Card>

      {totalCount > 0 && <div className="flex justify-end"><Button variant="ghost" size="sm" onClick={clearHistory} className="text-destructive"><Trash className="mr-1 h-4 w-4" />{t("history.clear.button")}</Button></div>}
    </SettingsPageLayout>
  );
}
