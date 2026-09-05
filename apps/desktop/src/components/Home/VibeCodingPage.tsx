import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Code, ArrowClockwise } from "@phosphor-icons/react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { useSettingsContext } from "@/contexts/SettingsContext";
import { platformQualityCommands, vibeCodingCommands, type VibeCodingStatus } from "@/lib/tauri";
import { showErrorToast } from "@/lib/toast";
import { SettingsPageLayout } from "./SettingsPageLayout";

export function VibeCodingPage() {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettingsContext();
  const [status, setStatus] = useState<VibeCodingStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const requestId = useRef(0);
  const refresh = useCallback(async () => {
    const id = ++requestId.current;
    try {
      const next = await vibeCodingCommands.getStatus();
      if (id === requestId.current) { setStatus(next); setError(null); }
    } catch (caught) { if (id === requestId.current) setError(String(caught)); }
  }, []);
  useEffect(() => {
    void refresh();
    window.addEventListener("focus", refresh);
    const timer = window.setInterval(refresh, 5000);
    return () => { ++requestId.current; window.clearInterval(timer); window.removeEventListener("focus", refresh); };
  }, [refresh]);
  const setEnabled = async (enabled: boolean) => {
    setBusy(true);
    ++requestId.current;
    try { await vibeCodingCommands.setEnabled(enabled); await refresh(); }
    catch (caught) { showErrorToast(String(caught)); }
    finally { setBusy(false); }
  };
  const setBridge = async (enabled: boolean) => {
    setBusy(true);
    try { await updateSetting("developer_bridge_enabled", enabled); await refresh(); }
    catch (caught) { showErrorToast(String(caught)); }
    finally { setBusy(false); }
  };
  const clear = async () => {
    setBusy(true);
    try { await platformQualityCommands.clearCodeContext(); await refresh(); }
    catch (caught) { showErrorToast(String(caught)); }
    finally { setBusy(false); }
  };
  const stateLabels = {
    disabled: t("vibe.disabled"), waiting_for_editor: t("vibe.waiting"), ready: t("vibe.ready"), stale: t("vibe.stale"),
  };
  return <SettingsPageLayout title={t("vibe.title")} description={t("vibe.description")} testId="vibe-coding-page">
    {error && <div role="alert" className="text-sm text-destructive">{error}<Button onClick={refresh} variant="ghost">{t("common.retry")}</Button></div>}
    {!status && !error && <p role="status">{t("common.loading")}</p>}
    <Card><CardContent className="flex items-center justify-between gap-5 pt-6 md:pt-6"><div className="space-y-2"><Label htmlFor="vibe-enabled">{t("vibe.enable")}</Label><p className="max-w-xl text-sm leading-6 text-muted-foreground">{t("vibe.scope")}</p></div><Switch id="vibe-enabled" disabled={busy || !status} checked={status?.enabled ?? false} onCheckedChange={setEnabled} /></CardContent></Card>
    {status && <Card><CardHeader className="flex-row items-center justify-between"><CardTitle className="flex items-center gap-2"><Code className="h-5 w-5" />{t("vibe.context")}</CardTitle><Button size="sm" variant="ghost" aria-label={t("vibe.refresh")} onClick={refresh}><ArrowClockwise className="h-4 w-4" /></Button></CardHeader><CardContent className="space-y-4">
      <p role="status" className="text-sm font-medium">{stateLabels[status.state]}</p>
      {status.file_name && <dl className="grid gap-4 text-sm sm:grid-cols-3">
        <div><dt className="text-xs text-muted-foreground">{t("vibe.editor")}</dt><dd className="mt-1">{status.editor ?? "—"}</dd></div>
        <div><dt className="text-xs text-muted-foreground">{t("vibe.file")}</dt><dd className="mt-1 break-all" title={status.file_path ?? undefined}>{status.file_name}</dd></div>
        <div><dt className="text-xs text-muted-foreground">{t("vibe.language")}</dt><dd className="mt-1">{status.language ?? "—"}</dd></div>
      </dl>}
      {status.identifiers.length > 0 && <div><h3 className="mb-2 text-xs text-muted-foreground">{t("vibe.identifiers")}</h3><div className="flex max-h-40 flex-wrap gap-2 overflow-y-auto">{status.identifiers.map(identifier => <code key={identifier} className="rounded-2xl bg-secondary px-3 py-1 text-xs">{identifier}</code>)}</div></div>}
      {status.updated_at_ms && <Button disabled={busy} variant="outline" size="sm" onClick={clear}>{t("vibe.clear")}</Button>}
    </CardContent></Card>}
    <Card><CardHeader><CardTitle>{t("vibe.setup")}</CardTitle></CardHeader><CardContent className="space-y-5">
      <p className="text-sm leading-7 text-muted-foreground">{t("vibe.setupDescription")}</p>
      <div className="flex items-center justify-between gap-5 rounded-2xl border border-border p-4"><div><Label htmlFor="vibe-bridge">{t("vibe.bridge")}</Label><p className="mt-1 text-xs leading-6 text-muted-foreground">{t("vibe.bridgeDescription")}</p></div><Switch id="vibe-bridge" checked={settings?.developer_bridge_enabled ?? false} disabled={busy || !settings} onCheckedChange={setBridge} /></div>
      <ol className="list-decimal space-y-3 pl-5 text-sm leading-7 text-muted-foreground"><li>{t("vibe.install")}</li><li>{t("vibe.configure")}</li><li>{t("vibe.connect")}</li></ol>
      <p className="text-xs leading-6 text-muted-foreground">{t("vibe.privacy")}</p>
    </CardContent></Card>
  </SettingsPageLayout>;
}
