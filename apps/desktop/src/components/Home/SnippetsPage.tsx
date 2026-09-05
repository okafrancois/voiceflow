import { useCallback, useEffect, useMemo, useState } from "react";
import { FloppyDisk, MagnifyingGlass, Plus, Trash } from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { useConfirm } from "@/components/ui/confirm";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { logger } from "@/lib/logger";
import { workflowCommands, type VoiceSnippet } from "@/lib/tauri";
import { showErrorToast, showToast } from "@/lib/toast";
import { SettingsPageLayout } from "@/components/Home/SettingsPageLayout";

const EMPTY_SNIPPET = {
  spoken_trigger: "",
  template: "",
};

function slugify(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "") || "snippet";
}

function uniqueId(trigger: string, snippets: VoiceSnippet[]): string {
  const base = slugify(trigger);
  const ids = new Set(snippets.map((snippet) => snippet.id));
  if (!ids.has(base)) return base;
  let suffix = 2;
  while (ids.has(`${base}-${suffix}`)) suffix += 1;
  return `${base}-${suffix}`;
}

export function SnippetsPage() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [snippets, setSnippets] = useState<VoiceSnippet[]>([]);
  const [draft, setDraft] = useState(EMPTY_SNIPPET);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>("load");
  const [loadError, setLoadError] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const snapshot = await workflowCommands.getSettings();
      setSnippets(snapshot.snippets.map((snippet) => ({ ...snippet })));
      setLoadError(false);
    } catch (error) {
      logger.error("snippets_page_load_failed", { error: String(error) });
      setLoadError(true);
      showErrorToast(t("snippetsPage.loadError"));
    } finally {
      setBusy(null);
    }
  }, [t]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const visibleSnippets = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return snippets;
    return snippets.filter((snippet) =>
      snippet.spoken_trigger.toLowerCase().includes(normalized)
      || snippet.template.toLowerCase().includes(normalized));
  }, [query, snippets]);

  const patchSnippet = (id: string, patch: Partial<VoiceSnippet>) => {
    setSnippets((current) => current.map((snippet) =>
      snippet.id === id ? { ...snippet, ...patch } : snippet));
  };

  const persist = async (snippet: VoiceSnippet, successMessage: string): Promise<boolean> => {
    setBusy(snippet.id);
    try {
      await workflowCommands.upsertVoiceSnippet(snippet);
      setSnippets((current) => current.some((item) => item.id === snippet.id)
        ? current.map((item) => item.id === snippet.id ? snippet : item)
        : [...current, snippet]);
      showToast(successMessage);
      setBusy(null);
      return true;
    } catch (error) {
      logger.error("snippet_save_failed", { id: snippet.id, error: String(error) });
      showErrorToast(t("snippetsPage.saveError"));
      setBusy(null);
      return false;
    }
  };

  const addSnippet = async () => {
    const trigger = draft.spoken_trigger.trim();
    const template = draft.template.trim();
    if (!trigger || !template) {
      showErrorToast(t("snippetsPage.requiredError"));
      return;
    }
    const snippet: VoiceSnippet = {
      id: uniqueId(trigger, snippets),
      spoken_trigger: trigger,
      template,
      enabled: true,
    };
    if (await persist(snippet, t("snippetsPage.added"))) {
      setDraft(EMPTY_SNIPPET);
    }
  };

  const deleteSnippet = async (snippet: VoiceSnippet) => {
    const accepted = await confirm({
      title: t("snippetsPage.deleteTitle"),
      description: t("snippetsPage.deleteDescription", { trigger: snippet.spoken_trigger }),
      confirmText: t("snippetsPage.deleteConfirm"),
      cancelText: t("snippetsPage.cancel"),
      variant: "danger",
    });
    if (!accepted) return;
    setBusy(snippet.id);
    try {
      await workflowCommands.deleteVoiceSnippet(snippet.id);
      setSnippets((current) => current.filter((item) => item.id !== snippet.id));
      showToast(t("snippetsPage.deleted"));
      setBusy(null);
    } catch (error) {
      logger.error("snippet_delete_failed", { id: snippet.id, error: String(error) });
      showErrorToast(t("snippetsPage.deleteError"));
      setBusy(null);
    }
  };

  return (
    <SettingsPageLayout
      title={t("snippetsPage.title")}
      description={t("snippetsPage.description")}
      testId="snippets-page"
    >
      <Card className="p-5 md:p-6">
        <div className="space-y-1">
          <h2 className="text-lg font-semibold">{t("snippetsPage.addTitle")}</h2>
          <p className="text-sm text-muted-foreground">{t("snippetsPage.variablesHelp")}</p>
        </div>
        <div className="mt-5 grid gap-4 md:grid-cols-2">
          <label className="space-y-2 text-sm font-medium">
            <span>{t("snippetsPage.triggerLabel")}</span>
            <Input
              aria-label={t("snippetsPage.triggerLabel")}
              value={draft.spoken_trigger}
              onChange={(event) => setDraft({ ...draft, spoken_trigger: event.target.value })}
              placeholder={t("snippetsPage.triggerPlaceholder")}
            />
          </label>
          <label className="space-y-2 text-sm font-medium">
            <span>{t("snippetsPage.contentLabel")}</span>
            <textarea
              aria-label={t("snippetsPage.contentLabel")}
              value={draft.template}
              onChange={(event) => setDraft({ ...draft, template: event.target.value })}
              placeholder={t("snippetsPage.contentPlaceholder")}
              className="min-h-24 w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm focus-visible:border-primary focus-visible:outline-none"
            />
          </label>
        </div>
        <Button className="mt-4" disabled={busy !== null} onClick={() => void addSnippet()}>
          <Plus className="mr-2 h-4 w-4" />
          {t("snippetsPage.add")}
        </Button>
      </Card>

      <div className="relative max-w-sm">
        <MagnifyingGlass className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("snippetsPage.searchPlaceholder")}
          className="rounded-full pl-9"
        />
      </div>

      {loadError ? (
        <Card className="p-6" role="alert">
          <p className="text-sm text-destructive">{t("snippetsPage.loadError")}</p>
          <Button className="mt-4" variant="outline" onClick={() => void refresh()}>
            {t("snippetsPage.retry")}
          </Button>
        </Card>
      ) : busy === "load" ? (
        <p role="status" className="text-sm text-muted-foreground">{t("snippetsPage.loading")}</p>
      ) : visibleSnippets.length === 0 ? (
        <Card className="p-10 text-center text-sm text-muted-foreground">
          {query ? t("snippetsPage.noResults") : t("snippetsPage.empty")}
        </Card>
      ) : (
        <div className="space-y-4">
          {visibleSnippets.map((snippet) => (
            <article key={snippet.id} className="rounded-3xl border border-border bg-card p-5 md:p-6">
              <div className="grid gap-4 md:grid-cols-[minmax(0,0.75fr)_minmax(0,1.25fr)]">
                <label className="space-y-2 text-sm font-medium">
                  <span>{t("snippetsPage.triggerLabel")}</span>
                  <Input
                    aria-label={t("snippetsPage.triggerLabel")}
                    value={snippet.spoken_trigger}
                    onChange={(event) => patchSnippet(snippet.id, { spoken_trigger: event.target.value })}
                  />
                </label>
                <label className="space-y-2 text-sm font-medium">
                  <span>{t("snippetsPage.contentLabel")}</span>
                  <textarea
                    aria-label={t("snippetsPage.contentLabel")}
                    value={snippet.template}
                    onChange={(event) => patchSnippet(snippet.id, { template: event.target.value })}
                    className="min-h-24 w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm focus-visible:border-primary focus-visible:outline-none"
                  />
                </label>
              </div>
              <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
                <label className="flex items-center gap-3 text-sm">
                  <Switch
                    checked={snippet.enabled}
                    disabled={busy !== null}
                    onCheckedChange={(enabled) => {
                      const updated = { ...snippet, enabled };
                      patchSnippet(snippet.id, { enabled });
                      void persist(updated, t("snippetsPage.saved"));
                    }}
                  />
                  <span>{t("snippetsPage.enabled")}</span>
                </label>
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    disabled={busy !== null}
                    onClick={() => void persist(snippet, t("snippetsPage.saved"))}
                  >
                    <FloppyDisk className="mr-2 h-4 w-4" />
                    {t("snippetsPage.save")}
                  </Button>
                  <Button
                    variant="ghost"
                    className="text-destructive"
                    disabled={busy !== null}
                    onClick={() => void deleteSnippet(snippet)}
                  >
                    <Trash className="mr-2 h-4 w-4" />
                    {t("snippetsPage.delete")}
                  </Button>
                </div>
              </div>
            </article>
          ))}
        </div>
      )}
    </SettingsPageLayout>
  );
}
