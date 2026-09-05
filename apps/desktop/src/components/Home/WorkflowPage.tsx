import { useLocation } from "react-router-dom";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ArrowsClockwise, Plus, Trash } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useConfirm } from "@/components/ui/confirm";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  workflowCommands,
  type ApplicationRule,
  type CapturedContext,
  type ContextCaptureSettings,
  type QuickControlKind,
  type VoiceActionKind,
  type VoiceActionPreview,
  type VoiceSnippet,
  type WorkflowOutputAction,
  type WorkflowProfile,
  type WorkflowSettingsSnapshot,
} from "@/lib/tauri";
import { showErrorToast, showToast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { SettingsPageLayout } from "./SettingsPageLayout";

type WorkflowTab = "context" | "profiles" | "rules" | "snippets" | "actions";
type ContextResultKey =
  | "application_id"
  | "application_name"
  | "window_title"
  | "focused_field_role"
  | "selected_text"
  | "clipboard_text"
  | "ocr_text";

const TABS: WorkflowTab[] = ["context", "profiles", "rules", "snippets", "actions"];
const TRIGGER_MODES: WorkflowProfile["trigger_mode"][] = ["hold", "toggle", "double_tap"];
const OUTPUT_ACTIONS: WorkflowOutputAction[] = ["insert", "preview", "copy"];
const VOICE_ACTIONS: VoiceActionKind[] = ["shorten", "translate", "reply", "list", "custom"];
const QUICK_CONTROLS: QuickControlKind[] = [
  "reinsert_raw",
  "reinsert_final",
  "copy_raw",
  "copy_final",
  "repolish",
  "submit_enter",
  "cancel_active_task",
];
const EMPTY_PROFILE: WorkflowProfile = {
  id: "",
  name: "",
  hotkey: "",
  trigger_mode: "hold",
  language: null,
  polish_template_id: null,
  translation_target: null,
  output_action: "insert",
  code_aware: false,
  protected: false,
};

const EMPTY_RULE: ApplicationRule = {
  id: "",
  application_id: "",
  title_contains: null,
  profile_id: "",
  enabled: true,
};

const EMPTY_SNIPPET: VoiceSnippet = {
  id: "",
  spoken_trigger: "",
  template: "",
  enabled: true,
};

function optionalValue(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">{children}</CardContent>
    </Card>
  );
}

function Field({
  label,
  id,
  value,
  onChange,
  disabled,
}: {
  label: string;
  id: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

export function WorkflowPage() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const location = useLocation();
  const requestedTab = new URLSearchParams(location.search).get("tab");
  const [tab, setTab] = useState<WorkflowTab>(() => TABS.find((candidate) => candidate === requestedTab) ?? "profiles");
  const [snapshot, setSnapshot] = useState<WorkflowSettingsSnapshot | null>(null);
  const [context, setContext] = useState<CapturedContext | null>(null);
  const [profileDraft, setProfileDraft] = useState<WorkflowProfile>(EMPTY_PROFILE);
  const [ruleDraft, setRuleDraft] = useState<ApplicationRule>(EMPTY_RULE);
  const [snippetDraft, setSnippetDraft] = useState<VoiceSnippet>(EMPTY_SNIPPET);
  const [snippetTrigger, setSnippetTrigger] = useState("");
  const [snippetPreview, setSnippetPreview] = useState<string | null>(null);
  const [actionKind, setActionKind] = useState<VoiceActionKind>("shorten");
  const [actionSource, setActionSource] = useState("");
  const [actionTarget, setActionTarget] = useState("");
  const [customInstruction, setCustomInstruction] = useState("");
  const [actionOutput, setActionOutput] = useState<WorkflowOutputAction>("preview");
  const [actionPreview, setActionPreview] = useState<VoiceActionPreview | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setBusy("load");
    try {
      const next = await workflowCommands.getSettings();
      setSnapshot(next);
    } catch (error) {
      showErrorToast(`${t("workflow.messages.loadError")} ${String(error)}`);
    } finally {
      setBusy(null);
    }
  }, [t]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(async (
    key: string,
    operation: () => Promise<void>,
    successMessage: string,
  ) => {
    setBusy(key);
    try {
      await operation();
      await refresh();
      showToast(successMessage);
    } catch (error) {
      showErrorToast(`${t("workflow.messages.saveError")} ${String(error)}`);
    } finally {
      setBusy(null);
    }
  }, [refresh, t]);

  const confirmDelete = useCallback(async (
    description: string,
    operation: () => Promise<void>,
    busyKey: string,
    successMessage: string,
  ) => {
    const accepted = await confirm({
      title: t("workflow.confirm.title"),
      description,
      confirmText: t("workflow.actions.delete"),
      cancelText: t("workflow.actions.cancel"),
      variant: "danger",
    });
    if (accepted) {
      await run(busyKey, operation, successMessage);
    }
  }, [confirm, run, t]);

  const updateContextCapture = async (
    key: keyof ContextCaptureSettings,
    checked: boolean,
  ) => {
    if (!snapshot) return;
    const next = { ...snapshot.context_capture, [key]: checked };
    setSnapshot({ ...snapshot, context_capture: next });
    await run(
      `context-${key}`,
      () => workflowCommands.setContextCapture(next),
      t("workflow.messages.contextSaved"),
    );
  };

  const captureContext = async () => {
    setBusy("capture");
    try {
      setContext(await workflowCommands.captureContext());
    } catch (error) {
      showErrorToast(`${t("workflow.messages.captureError")} ${String(error)}`);
    } finally {
      setBusy(null);
    }
  };

  const patchProfile = (id: string, patch: Partial<WorkflowProfile>) => {
    if (!snapshot) return;
    setSnapshot({
      ...snapshot,
      profiles: snapshot.profiles.map((profile) =>
        profile.id === id ? { ...profile, ...patch } : profile,
      ),
    });
  };

  const patchRule = (id: string, patch: Partial<ApplicationRule>) => {
    if (!snapshot) return;
    setSnapshot({
      ...snapshot,
      application_rules: snapshot.application_rules.map((rule) =>
        rule.id === id ? { ...rule, ...patch } : rule,
      ),
    });
  };

  const patchSnippet = (id: string, patch: Partial<VoiceSnippet>) => {
    if (!snapshot) return;
    setSnapshot({
      ...snapshot,
      snippets: snapshot.snippets.map((snippet) =>
        snippet.id === id ? { ...snippet, ...patch } : snippet,
      ),
    });
  };

  const profileOptions = useMemo(
    () => snapshot?.profiles.map((profile) => ({ value: profile.id, label: profile.name })) ?? [],
    [snapshot?.profiles],
  );
  const tabLabels: Record<WorkflowTab, string> = {
    context: t("workflow.tabs.context"),
    profiles: t("workflow.tabs.profiles"),
    rules: t("workflow.tabs.rules"),
    snippets: t("workflow.tabs.snippets"),
    actions: t("workflow.tabs.actions"),
  };
  const contextFieldLabels: Record<keyof ContextCaptureSettings, string> = {
    application_metadata: t("workflow.context.fields.application_metadata"),
    focused_field: t("workflow.context.fields.focused_field"),
    selected_text: t("workflow.context.fields.selected_text"),
    clipboard: t("workflow.context.fields.clipboard"),
    ocr_fallback: t("workflow.context.fields.ocr_fallback"),
  };
  const contextResultLabels: Record<ContextResultKey, string> = {
    application_id: t("workflow.context.result.application_id"),
    application_name: t("workflow.context.result.application_name"),
    window_title: t("workflow.context.result.window_title"),
    focused_field_role: t("workflow.context.result.focused_field_role"),
    selected_text: t("workflow.context.result.selected_text"),
    clipboard_text: t("workflow.context.result.clipboard_text"),
    ocr_text: t("workflow.context.result.ocr_text"),
  };
  const triggerLabels: Record<WorkflowProfile["trigger_mode"], string> = {
    hold: t("workflow.trigger.hold"),
    toggle: t("workflow.trigger.toggle"),
    double_tap: t("workflow.trigger.double_tap"),
  };
  const outputLabels: Record<WorkflowOutputAction, string> = {
    insert: t("workflow.output.insert"),
    preview: t("workflow.output.preview"),
    copy: t("workflow.output.copy"),
  };
  const voiceActionLabels: Record<VoiceActionKind, string> = {
    shorten: t("workflow.voiceActions.kinds.shorten"),
    translate: t("workflow.voiceActions.kinds.translate"),
    reply: t("workflow.voiceActions.kinds.reply"),
    list: t("workflow.voiceActions.kinds.list"),
    custom: t("workflow.voiceActions.kinds.custom"),
  };
  const quickControlLabels: Record<QuickControlKind, string> = {
    reinsert_raw: t("workflow.quick.reinsert_raw"),
    reinsert_final: t("workflow.quick.reinsert_final"),
    copy_raw: t("workflow.quick.copy_raw"),
    copy_final: t("workflow.quick.copy_final"),
    repolish: t("workflow.quick.repolish"),
    submit_enter: t("workflow.quick.submit_enter"),
    cancel_active_task: t("workflow.quick.cancel_active_task"),
  };

  if (!snapshot) {
    return (
      <SettingsPageLayout title={t("workflow.title")} description={t("workflow.description")}>
        <p className="text-sm text-muted-foreground">{t("workflow.loading")}</p>
      </SettingsPageLayout>
    );
  }

  return (
    <SettingsPageLayout
      title={t("workflow.title")}
      description={t("workflow.description")}
      testId="workflow-page"
    >
      <div className="flex flex-wrap gap-2" role="tablist" aria-label={t("workflow.tabs.label")}>
        {TABS.map((item) => (
          <Button
            key={item}
            type="button"
            role="tab"
            aria-selected={tab === item}
            variant={tab === item ? "default" : "outline"}
            onClick={() => setTab(item)}
          >
            {tabLabels[item]}
          </Button>
        ))}
      </div>

      {tab === "context" && (
        <div className="grid gap-5 lg:grid-cols-2">
          <Section title={t("workflow.context.title")} description={t("workflow.context.description")}>
            {(
              [
                "application_metadata",
                "focused_field",
                "selected_text",
                "clipboard",
                "ocr_fallback",
              ] as (keyof ContextCaptureSettings)[]
            ).map((key) => (
              <div key={key} className="flex items-center justify-between gap-4 rounded-2xl border border-border p-4">
                <Label htmlFor={`context-${key}`}>{contextFieldLabels[key]}</Label>
                <Switch
                  id={`context-${key}`}
                  checked={snapshot.context_capture[key]}
                  disabled={busy !== null}
                  onCheckedChange={(checked) => void updateContextCapture(key, checked)}
                />
              </div>
            ))}
          </Section>
          <Section title={t("workflow.context.previewTitle")} description={t("workflow.context.previewDescription")}>
            <Button type="button" onClick={() => void captureContext()} disabled={busy !== null}>
              <ArrowsClockwise className="mr-2 h-4 w-4" />
              {t("workflow.context.capture")}
            </Button>
            {context ? (
              <dl className="grid gap-3 text-sm">
                {([
                  "application_id",
                  "application_name",
                  "window_title",
                  "focused_field_role",
                  "selected_text",
                  "clipboard_text",
                  "ocr_text",
                ] as const).map((key) => (
                  <div key={key} className="grid gap-1 rounded-2xl bg-secondary/30 p-3">
                    <dt className="font-medium">{contextResultLabels[key]}</dt>
                    <dd className="whitespace-pre-wrap text-muted-foreground">
                      {context[key] ?? t("workflow.context.unavailable")}
                    </dd>
                  </div>
                ))}
              </dl>
            ) : (
              <p className="text-sm text-muted-foreground">{t("workflow.context.empty")}</p>
            )}
          </Section>
        </div>
      )}

      {tab === "profiles" && (
        <div className="space-y-5">
          {snapshot.profiles.map((profile) => (
            <Section
              key={profile.id}
              title={profile.name}
              description={profile.protected ? t("workflow.profiles.protected") : profile.id}
            >
              <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                <Field label={t("workflow.profiles.fields.name")} id={`profile-name-${profile.id}`} value={profile.name} onChange={(name) => patchProfile(profile.id, { name })} />
                <Field label={t("workflow.profiles.fields.hotkey")} id={`profile-hotkey-${profile.id}`} value={profile.hotkey} onChange={(hotkey) => patchProfile(profile.id, { hotkey })} />
                <div className="space-y-2">
                  <Label>{t("workflow.profiles.fields.trigger")}</Label>
                  <Select aria-label={t("workflow.profiles.fields.trigger")} value={profile.trigger_mode} options={TRIGGER_MODES.map((value) => ({ value, label: triggerLabels[value] }))} onChange={(event) => patchProfile(profile.id, { trigger_mode: event.target.value as WorkflowProfile["trigger_mode"] })} />
                </div>
                <Field label={t("workflow.profiles.fields.language")} id={`profile-language-${profile.id}`} value={profile.language ?? ""} onChange={(value) => patchProfile(profile.id, { language: optionalValue(value) })} />
                <Field label={t("workflow.profiles.fields.template")} id={`profile-template-${profile.id}`} value={profile.polish_template_id ?? ""} onChange={(value) => patchProfile(profile.id, { polish_template_id: optionalValue(value) })} />
                <Field label={t("workflow.profiles.fields.translation")} id={`profile-translation-${profile.id}`} value={profile.translation_target ?? ""} onChange={(value) => patchProfile(profile.id, { translation_target: optionalValue(value) })} />
                <div className="space-y-2">
                  <Label>{t("workflow.profiles.fields.output")}</Label>
                  <Select aria-label={t("workflow.profiles.fields.output")} value={profile.output_action} options={OUTPUT_ACTIONS.map((value) => ({ value, label: outputLabels[value] }))} onChange={(event) => patchProfile(profile.id, { output_action: event.target.value as WorkflowOutputAction })} />
                </div>
                <div className="flex items-center gap-3 pt-7">
                  <Switch id={`profile-code-${profile.id}`} checked={profile.code_aware} onCheckedChange={(code_aware) => patchProfile(profile.id, { code_aware })} />
                  <Label htmlFor={`profile-code-${profile.id}`}>{t("workflow.profiles.fields.codeAware")}</Label>
                </div>
              </div>
              <div className="flex gap-2">
                <Button type="button" disabled={busy !== null} onClick={() => void run(`profile-${profile.id}`, () => workflowCommands.updateProfile(profile), t("workflow.messages.profileSaved"))}>{t("workflow.actions.save")}</Button>
                {!profile.protected && (
                  <Button type="button" variant="outline" className="text-destructive" disabled={busy !== null} onClick={() => void confirmDelete(t("workflow.confirm.profile", { name: profile.name }), () => workflowCommands.deleteProfile(profile.id), `delete-profile-${profile.id}`, t("workflow.messages.profileDeleted"))}>
                    <Trash className="mr-2 h-4 w-4" />{t("workflow.actions.delete")}
                  </Button>
                )}
              </div>
            </Section>
          ))}
          <Section title={t("workflow.profiles.addTitle")} description={t("workflow.profiles.addDescription")}>
            <div className="grid gap-4 md:grid-cols-3">
              <Field label={t("workflow.profiles.fields.id")} id="new-profile-id" value={profileDraft.id} onChange={(id) => setProfileDraft({ ...profileDraft, id })} />
              <Field label={t("workflow.profiles.fields.name")} id="new-profile-name" value={profileDraft.name} onChange={(name) => setProfileDraft({ ...profileDraft, name })} />
              <Field label={t("workflow.profiles.fields.hotkey")} id="new-profile-hotkey" value={profileDraft.hotkey} onChange={(hotkey) => setProfileDraft({ ...profileDraft, hotkey })} />
            </div>
            <Button type="button" disabled={busy !== null} onClick={() => void run("create-profile", async () => { await workflowCommands.createProfile(profileDraft); setProfileDraft(EMPTY_PROFILE); }, t("workflow.messages.profileCreated"))}>
              <Plus className="mr-2 h-4 w-4" />{t("workflow.actions.add")}
            </Button>
          </Section>
        </div>
      )}

      {tab === "rules" && (
        <div className="space-y-5">
          {snapshot.application_rules.map((rule) => (
            <Section key={rule.id} title={rule.application_id} description={rule.id}>
              <div className="grid gap-4 md:grid-cols-2">
                <Field label={t("workflow.rules.fields.application")} id={`rule-app-${rule.id}`} value={rule.application_id} onChange={(application_id) => patchRule(rule.id, { application_id })} />
                <Field label={t("workflow.rules.fields.title")} id={`rule-title-${rule.id}`} value={rule.title_contains ?? ""} onChange={(value) => patchRule(rule.id, { title_contains: optionalValue(value) })} />
                <div className="space-y-2">
                  <Label>{t("workflow.rules.fields.profile")}</Label>
                  <Select aria-label={t("workflow.rules.fields.profile")} value={rule.profile_id} options={profileOptions} onChange={(event) => patchRule(rule.id, { profile_id: event.target.value })} />
                </div>
                <div className="flex items-center gap-3 pt-7"><Switch id={`rule-enabled-${rule.id}`} checked={rule.enabled} onCheckedChange={(enabled) => patchRule(rule.id, { enabled })} /><Label htmlFor={`rule-enabled-${rule.id}`}>{t("workflow.rules.fields.enabled")}</Label></div>
              </div>
              <div className="flex gap-2">
                <Button type="button" onClick={() => void run(`rule-${rule.id}`, () => workflowCommands.upsertApplicationRule(rule), t("workflow.messages.ruleSaved"))}>{t("workflow.actions.save")}</Button>
                <Button type="button" variant="outline" className="text-destructive" onClick={() => void confirmDelete(t("workflow.confirm.rule", { name: rule.application_id }), () => workflowCommands.deleteApplicationRule(rule.id), `delete-rule-${rule.id}`, t("workflow.messages.ruleDeleted"))}><Trash className="mr-2 h-4 w-4" />{t("workflow.actions.delete")}</Button>
              </div>
            </Section>
          ))}
          <Section title={t("workflow.rules.addTitle")} description={t("workflow.rules.description")}>
            <div className="grid gap-4 md:grid-cols-2">
              <Field label={t("workflow.rules.fields.id")} id="new-rule-id" value={ruleDraft.id} onChange={(id) => setRuleDraft({ ...ruleDraft, id })} />
              <Field label={t("workflow.rules.fields.application")} id="new-rule-app" value={ruleDraft.application_id} onChange={(application_id) => setRuleDraft({ ...ruleDraft, application_id })} />
              <Field label={t("workflow.rules.fields.title")} id="new-rule-title" value={ruleDraft.title_contains ?? ""} onChange={(value) => setRuleDraft({ ...ruleDraft, title_contains: optionalValue(value) })} />
              <div className="space-y-2"><Label>{t("workflow.rules.fields.profile")}</Label><Select aria-label={t("workflow.rules.fields.profile")} value={ruleDraft.profile_id} options={profileOptions} onChange={(event) => setRuleDraft({ ...ruleDraft, profile_id: event.target.value })} /></div>
            </div>
            <Button type="button" onClick={() => void run("create-rule", async () => { await workflowCommands.upsertApplicationRule(ruleDraft); setRuleDraft(EMPTY_RULE); }, t("workflow.messages.ruleSaved"))}><Plus className="mr-2 h-4 w-4" />{t("workflow.actions.add")}</Button>
          </Section>
        </div>
      )}

      {tab === "snippets" && (
        <div className="space-y-5">
          {snapshot.snippets.map((snippet) => (
            <Section key={snippet.id} title={snippet.spoken_trigger} description={snippet.id}>
              <div className="grid gap-4 md:grid-cols-2">
                <Field label={t("workflow.snippets.fields.trigger")} id={`snippet-trigger-${snippet.id}`} value={snippet.spoken_trigger} onChange={(spoken_trigger) => patchSnippet(snippet.id, { spoken_trigger })} />
                <Field label={t("workflow.snippets.fields.template")} id={`snippet-template-${snippet.id}`} value={snippet.template} onChange={(template) => patchSnippet(snippet.id, { template })} />
              </div>
              <div className="flex items-center gap-3"><Switch id={`snippet-enabled-${snippet.id}`} checked={snippet.enabled} onCheckedChange={(enabled) => patchSnippet(snippet.id, { enabled })} /><Label htmlFor={`snippet-enabled-${snippet.id}`}>{t("workflow.snippets.fields.enabled")}</Label></div>
              <div className="flex gap-2"><Button type="button" onClick={() => void run(`snippet-${snippet.id}`, () => workflowCommands.upsertVoiceSnippet(snippet), t("workflow.messages.snippetSaved"))}>{t("workflow.actions.save")}</Button><Button type="button" variant="outline" className="text-destructive" onClick={() => void confirmDelete(t("workflow.confirm.snippet", { name: snippet.spoken_trigger }), () => workflowCommands.deleteVoiceSnippet(snippet.id), `delete-snippet-${snippet.id}`, t("workflow.messages.snippetDeleted"))}><Trash className="mr-2 h-4 w-4" />{t("workflow.actions.delete")}</Button></div>
            </Section>
          ))}
          <Section title={t("workflow.snippets.addTitle")} description={t("workflow.snippets.description")}>
            <div className="grid gap-4 md:grid-cols-3">
              <Field label={t("workflow.snippets.fields.id")} id="new-snippet-id" value={snippetDraft.id} onChange={(id) => setSnippetDraft({ ...snippetDraft, id })} />
              <Field label={t("workflow.snippets.fields.trigger")} id="new-snippet-trigger" value={snippetDraft.spoken_trigger} onChange={(spoken_trigger) => setSnippetDraft({ ...snippetDraft, spoken_trigger })} />
              <Field label={t("workflow.snippets.fields.template")} id="new-snippet-template" value={snippetDraft.template} onChange={(template) => setSnippetDraft({ ...snippetDraft, template })} />
            </div>
            <Button type="button" onClick={() => void run("create-snippet", async () => { await workflowCommands.upsertVoiceSnippet(snippetDraft); setSnippetDraft(EMPTY_SNIPPET); }, t("workflow.messages.snippetSaved"))}><Plus className="mr-2 h-4 w-4" />{t("workflow.actions.add")}</Button>
          </Section>
          <Section title={t("workflow.snippets.testTitle")} description={t("workflow.snippets.testDescription")}>
            <Field label={t("workflow.snippets.testTrigger")} id="snippet-test-trigger" value={snippetTrigger} onChange={setSnippetTrigger} />
            <Button type="button" onClick={() => { setBusy("snippet-test"); workflowCommands.expandVoiceSnippet(snippetTrigger).then(setSnippetPreview).catch((error: unknown) => showErrorToast(String(error))).finally(() => setBusy(null)); }}>{t("workflow.snippets.test")}</Button>
            {snippetPreview !== null && <pre className="whitespace-pre-wrap rounded-2xl bg-secondary/30 p-4 text-sm">{snippetPreview}</pre>}
          </Section>
        </div>
      )}

      {tab === "actions" && (
        <div className="grid gap-5 lg:grid-cols-2">
          <Section title={t("workflow.voiceActions.title")} description={t("workflow.voiceActions.description")}>
            <div className="space-y-2"><Label>{t("workflow.voiceActions.kind")}</Label><Select aria-label={t("workflow.voiceActions.kind")} value={actionKind} options={VOICE_ACTIONS.map((value) => ({ value, label: voiceActionLabels[value] }))} onChange={(event) => setActionKind(event.target.value as VoiceActionKind)} /></div>
            <div className="space-y-2"><Label htmlFor="voice-action-source">{t("workflow.voiceActions.source")}</Label><textarea id="voice-action-source" className={cn("min-h-32 w-full rounded-2xl border border-border bg-background p-4 text-sm focus-visible:border-primary focus-visible:outline-none")} value={actionSource} onChange={(event) => setActionSource(event.target.value)} /></div>
            {actionKind === "translate" && <Field label={t("workflow.voiceActions.target")} id="voice-action-target" value={actionTarget} onChange={setActionTarget} />}
            {actionKind === "custom" && <Field label={t("workflow.voiceActions.instruction")} id="voice-action-instruction" value={customInstruction} onChange={setCustomInstruction} />}
            <div className="space-y-2"><Label>{t("workflow.voiceActions.output")}</Label><Select aria-label={t("workflow.voiceActions.output")} value={actionOutput} options={OUTPUT_ACTIONS.filter((value) => value !== "insert").map((value) => ({ value, label: outputLabels[value] }))} onChange={(event) => setActionOutput(event.target.value as WorkflowOutputAction)} /></div>
            <Button type="button" disabled={busy !== null} onClick={() => { setBusy("voice-action"); workflowCommands.runVoiceAction({ kind: actionKind, selected_text: optionalValue(actionSource), translation_target: optionalValue(actionTarget), custom_instruction: optionalValue(customInstruction), output_action: actionOutput }).then(setActionPreview).catch((error: unknown) => showErrorToast(`${t("workflow.messages.actionError")} ${String(error)}`)).finally(() => setBusy(null)); }}>{t("workflow.voiceActions.run")}</Button>
            {actionPreview && <div className="space-y-3 rounded-2xl bg-secondary/30 p-4"><p className="whitespace-pre-wrap text-sm">{actionPreview.result_text}</p><Button type="button" onClick={() => { setBusy("replace-preview"); workflowCommands.replaceVoiceActionPreview().then(setActionPreview).catch((error: unknown) => showErrorToast(String(error))).finally(() => setBusy(null)); }}>{t("workflow.voiceActions.replace")}</Button></div>}
          </Section>
          <Section title={t("workflow.quick.title")} description={t("workflow.quick.description")}>
            <div className="grid gap-2 sm:grid-cols-2">
              {QUICK_CONTROLS.map((control) => (
                <Button key={control} type="button" variant="outline" disabled={busy !== null} onClick={() => { setBusy(`quick-${control}`); workflowCommands.runQuickControl(control).then((result) => { if (result.text) setActionPreview({ kind: "custom", source_text: "", result_text: result.text, translation_target: null, output_action: "preview" }); showToast(t("workflow.messages.quickDone")); }).catch((error: unknown) => showErrorToast(String(error))).finally(() => setBusy(null)); }}>{quickControlLabels[control]}</Button>
              ))}
            </div>
          </Section>
        </div>
      )}
    </SettingsPageLayout>
  );
}
