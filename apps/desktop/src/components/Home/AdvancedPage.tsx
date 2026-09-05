import { useState } from "react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { useSettingsContext } from "@/contexts/SettingsContext";
import { showErrorToast } from "@/lib/toast";
import { SettingsPageLayout } from "./SettingsPageLayout";

function toolLinks(t: (key: string) => string) { return [
  { id: "profiles", title: t("advanced.profiles.title"), path: "/workflows?tab=profiles" },
  { id: "templates", title: t("advanced.templates.title"), path: "/polish-templates" },
  { id: "actions", title: t("advanced.actions.title"), path: "/workflows?tab=actions" },
  { id: "media", title: t("advanced.media.title"), path: "/workbench" },
  { id: "diagnostics", title: t("advanced.diagnostics.title"), path: "/quality" },
] as const; }

export function AdvancedPage() {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettingsContext();
  const [busy, setBusy] = useState(false);
  const setBridge = async (enabled: boolean) => {
    setBusy(true);
    try { await updateSetting("developer_bridge_enabled", enabled); }
    catch (error) { showErrorToast(String(error)); }
    finally { setBusy(false); }
  };
  return <SettingsPageLayout title={t("advanced.title")} description={t("advanced.description")} testId="advanced-page">
    <div className="grid gap-4 md:grid-cols-2">{toolLinks(t).map((tool) => <Card key={tool.id}>
      <CardHeader><CardTitle><Link className="underline-offset-4 hover:underline focus-visible:underline" to={tool.path}>{tool.title}</Link></CardTitle></CardHeader>
    </Card>)}</div>
    <Card><CardHeader><CardTitle>{t("advanced.bridge.title")}</CardTitle><CardDescription>{t("advanced.bridge.description")}</CardDescription></CardHeader>
      <CardContent className="flex items-center justify-between gap-4"><Label htmlFor="developer-bridge">{t("advanced.bridge.enable")}</Label><Switch id="developer-bridge" disabled={busy || !settings} checked={settings?.developer_bridge_enabled ?? false} onCheckedChange={setBridge} /></CardContent>
    </Card>
  </SettingsPageLayout>;
}
