import { useCallback, useEffect, useState } from "react";
import { FloppyDisk, Plus, Trash } from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";

import { SettingsPageLayout } from "@/components/Home/SettingsPageLayout";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { useConfirm } from "@/components/ui/confirm";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { logger } from "@/lib/logger";
import {
  modelCommands,
  systemCommands,
  workflowCommands,
  type ApplicationRule,
  type CustomPolishTemplate,
  type WorkflowProfile,
} from "@/lib/tauri";
import { showErrorToast, showToast } from "@/lib/toast";

const EMPTY_ASSIGNMENT = {
  application_id: "",
  custom_application_id: "",
  title_contains: "",
  profile_id: "",
};

const EMPTY_STYLE = {
  name: "",
  hotkey: "",
  polish_template_id: "",
};

const CUSTOM_APPLICATION = "__custom__";
const MAC_APPLICATIONS = [
  { id: "com.microsoft.VSCode", name: "VS Code" },
  { id: "com.todesktop.230313mzl4w4u92", name: "Cursor" },
  { id: "com.exafunction.windsurf", name: "Windsurf" },
  { id: "com.apple.mail", name: "Mail" },
  { id: "com.tinyspeck.slackmacgap", name: "Slack" },
  { id: "com.apple.Safari", name: "Safari" },
  { id: "com.google.Chrome", name: "Google Chrome" },
] as const;

function slugify(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "") || "application";
}

function uniqueRuleId(applicationId: string, rules: ApplicationRule[]): string {
  const base = slugify(applicationId);
  const ids = new Set(rules.map((rule) => rule.id));
  if (!ids.has(base)) return base;
  let suffix = 2;
  while (ids.has(`${base}-${suffix}`)) suffix += 1;
  return `${base}-${suffix}`;
}

function uniqueProfileId(name: string, profiles: WorkflowProfile[]): string {
  const base = slugify(name);
  const ids = new Set(profiles.map((profile) => profile.id));
  if (!ids.has(base)) return base;
  let suffix = 2;
  while (ids.has(`${base}-${suffix}`)) suffix += 1;
  return `${base}-${suffix}`;
}

export function StylesPage() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [profiles, setProfiles] = useState<WorkflowProfile[]>([]);
  const [rules, setRules] = useState<ApplicationRule[]>([]);
  const [customTemplates, setCustomTemplates] = useState<CustomPolishTemplate[]>([]);
  const [platform, setPlatform] = useState<"macos" | "windows" | "linux" | "unknown">("unknown");
  const [newStyle, setNewStyle] = useState(EMPTY_STYLE);
  const [assignment, setAssignment] = useState(EMPTY_ASSIGNMENT);
  const [busy, setBusy] = useState<string | null>("load");
  const [loadError, setLoadError] = useState(false);
  const templateOptions = [
    { value: "", label: t("stylesPage.noPolish") },
    { value: "filler", label: t("model.polish.templateFiller") },
    { value: "chat", label: t("model.polish.templateChat") },
    { value: "formal", label: t("model.polish.templateFormal") },
    { value: "concise", label: t("model.polish.templateConcise") },
    { value: "document", label: t("model.polish.templateDocument") },
    { value: "agent", label: t("model.polish.templateAgent") },
    ...customTemplates.map((template) => ({ value: template.id, label: template.name })),
  ];
  const applicationOptions = platform === "macos" ? MAC_APPLICATIONS : [];

  const refresh = useCallback(async () => {
    try {
      const [snapshot, templates, currentPlatform] = await Promise.all([
        workflowCommands.getSettings(),
        modelCommands.getPolishCustomTemplates(),
        systemCommands.getPlatform(),
      ]);
      setProfiles(snapshot.profiles.map((profile) => ({ ...profile })));
      setRules(snapshot.application_rules.map((rule) => ({ ...rule })));
      setCustomTemplates(templates);
      setPlatform(currentPlatform);
      setLoadError(false);
    } catch (error) {
      logger.error("styles_page_load_failed", { error: String(error) });
      setLoadError(true);
      showErrorToast(t("stylesPage.loadError"));
    } finally {
      setBusy(null);
    }
  }, [t]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const patchProfile = (id: string, patch: Partial<WorkflowProfile>) => {
    setProfiles((current) => current.map((profile) =>
      profile.id === id ? { ...profile, ...patch } : profile));
  };

  const patchRule = (id: string, patch: Partial<ApplicationRule>) => {
    setRules((current) => current.map((rule) =>
      rule.id === id ? { ...rule, ...patch } : rule));
  };

  const saveProfile = async (profile: WorkflowProfile) => {
    setBusy(profile.id);
    try {
      await workflowCommands.updateProfile(profile);
      showToast(t("stylesPage.profileSaved"));
      setBusy(null);
    } catch (error) {
      logger.error("style_profile_save_failed", { id: profile.id, error: String(error) });
      showErrorToast(t("stylesPage.saveError"));
      setBusy(null);
    }
  };

  const createStyle = async () => {
    const name = newStyle.name.trim();
    const hotkey = newStyle.hotkey.trim();
    if (!name) {
      showErrorToast(t("stylesPage.createRequiredError"));
      return;
    }
    const profile: WorkflowProfile = {
      id: uniqueProfileId(name, profiles),
      name,
      hotkey,
      trigger_mode: "hold",
      language: null,
      polish_template_id: newStyle.polish_template_id || null,
      translation_target: null,
      output_action: "insert",
      code_aware: false,
      protected: false,
    };
    setBusy("create-style");
    try {
      await workflowCommands.createProfile(profile);
      setProfiles((current) => [...current, profile]);
      setNewStyle(EMPTY_STYLE);
      showToast(t("stylesPage.styleCreated"));
      setBusy(null);
    } catch (error) {
      logger.error("style_profile_create_failed", { id: profile.id, error: String(error) });
      showErrorToast(t("stylesPage.saveError"));
      setBusy(null);
    }
  };

  const saveRule = async (rule: ApplicationRule, successMessage: string): Promise<boolean> => {
    setBusy(rule.id);
    try {
      await workflowCommands.upsertApplicationRule(rule);
      setRules((current) => current.some((item) => item.id === rule.id)
        ? current.map((item) => item.id === rule.id ? rule : item)
        : [...current, rule]);
      showToast(successMessage);
      setBusy(null);
      return true;
    } catch (error) {
      logger.error("style_assignment_save_failed", { id: rule.id, error: String(error) });
      showErrorToast(t("stylesPage.saveError"));
      setBusy(null);
      return false;
    }
  };

  const addAssignment = async () => {
    const applicationId = (assignment.application_id === CUSTOM_APPLICATION
      ? assignment.custom_application_id
      : assignment.application_id).trim();
    const profileId = assignment.profile_id || profiles[0]?.id;
    if (!applicationId || !profileId) {
      showErrorToast(t("stylesPage.requiredError"));
      return;
    }
    const rule: ApplicationRule = {
      id: uniqueRuleId(applicationId, rules),
      application_id: applicationId,
      title_contains: assignment.title_contains.trim() || null,
      profile_id: profileId,
      enabled: true,
    };
    if (await saveRule(rule, t("stylesPage.assignmentAdded"))) {
      setAssignment(EMPTY_ASSIGNMENT);
    }
  };

  const deleteRule = async (rule: ApplicationRule) => {
    const accepted = await confirm({
      title: t("stylesPage.deleteTitle"),
      description: t("stylesPage.deleteDescription", { application: rule.application_id }),
      confirmText: t("stylesPage.deleteConfirm"),
      cancelText: t("stylesPage.cancel"),
      variant: "danger",
    });
    if (!accepted) return;
    setBusy(rule.id);
    try {
      await workflowCommands.deleteApplicationRule(rule.id);
      setRules((current) => current.filter((item) => item.id !== rule.id));
      showToast(t("stylesPage.assignmentDeleted"));
      setBusy(null);
    } catch (error) {
      logger.error("style_assignment_delete_failed", { id: rule.id, error: String(error) });
      showErrorToast(t("stylesPage.deleteError"));
      setBusy(null);
    }
  };

  if (loadError) {
    return (
      <SettingsPageLayout title={t("stylesPage.title")} description={t("stylesPage.description")} testId="styles-page">
        <Card className="p-6" role="alert">
          <p className="text-sm text-destructive">{t("stylesPage.loadError")}</p>
          <Button className="mt-4" variant="outline" onClick={() => void refresh()}>{t("stylesPage.retry")}</Button>
        </Card>
      </SettingsPageLayout>
    );
  }

  return (
    <SettingsPageLayout title={t("stylesPage.title")} description={t("stylesPage.description")} testId="styles-page">
      {busy === "load" ? (
        <p role="status" className="text-sm text-muted-foreground">{t("stylesPage.loading")}</p>
      ) : (
        <>
          <section className="space-y-4">
            <div className="space-y-1">
              <h2 className="text-lg font-semibold">{t("stylesPage.profilesTitle")}</h2>
              <p className="text-sm text-muted-foreground">{t("stylesPage.profilesDescription")}</p>
            </div>
            <Card className="p-5 md:p-6">
              <div className="space-y-1">
                <h3 className="font-semibold">{t("stylesPage.createTitle")}</h3>
                <p className="text-sm text-muted-foreground">{t("stylesPage.createDescription")}</p>
              </div>
              <div className="mt-4 grid gap-4 md:grid-cols-3">
                <label className="space-y-2 text-sm font-medium">
                  <span>{t("stylesPage.nameLabel")}</span>
                  <Input
                    aria-label={t("stylesPage.createNameLabel")}
                    value={newStyle.name}
                    onChange={(event) => setNewStyle({ ...newStyle, name: event.target.value })}
                  />
                </label>
                <label className="space-y-2 text-sm font-medium">
                  <span>{t("stylesPage.hotkeyLabel")}</span>
                  <Input
                    aria-label={t("stylesPage.hotkeyLabel")}
                    value={newStyle.hotkey}
                    onChange={(event) => setNewStyle({ ...newStyle, hotkey: event.target.value })}
                    placeholder={t("stylesPage.hotkeyPlaceholder")}
                  />
                </label>
                <label className="space-y-2 text-sm font-medium">
                  <span>{t("stylesPage.templateLabel")}</span>
                  <select
                    aria-label={t("stylesPage.createTemplateLabel")}
                    value={newStyle.polish_template_id}
                    onChange={(event) => setNewStyle({ ...newStyle, polish_template_id: event.target.value })}
                    className="h-10 w-full rounded-2xl border border-border bg-background px-4 text-sm"
                  >
                    {templateOptions.map((option) => (
                      <option key={option.value} value={option.value}>{option.label}</option>
                    ))}
                  </select>
                </label>
              </div>
              <Button className="mt-4" disabled={busy !== null} onClick={() => void createStyle()}>
                <Plus className="mr-2 h-4 w-4" />
                {t("stylesPage.createStyle")}
              </Button>
            </Card>
            {profiles.map((profile) => {
              const knownTemplate = templateOptions.some((option) => option.value === (profile.polish_template_id ?? ""));
              return (
                <article key={profile.id} className="rounded-3xl border border-border bg-card p-5 md:p-6">
                  <h3 className="mb-4 font-semibold">{profile.name}</h3>
                  <div className="grid gap-4 md:grid-cols-2">
                    <label className="space-y-2 text-sm font-medium">
                      <span>{t("stylesPage.nameLabel")}</span>
                      <Input
                        aria-label={t("stylesPage.nameLabel")}
                        value={profile.name}
                        onChange={(event) => patchProfile(profile.id, { name: event.target.value })}
                      />
                    </label>
                    <label className="space-y-2 text-sm font-medium">
                      <span>{t("stylesPage.templateLabel")}</span>
                      <select
                        aria-label={t("stylesPage.templateLabel")}
                        value={profile.polish_template_id ?? ""}
                        onChange={(event) => patchProfile(profile.id, { polish_template_id: event.target.value || null })}
                        className="h-10 w-full rounded-2xl border border-border bg-background px-4 text-sm"
                      >
                        {!knownTemplate && profile.polish_template_id && (
                          <option value={profile.polish_template_id}>{t("model.polish.templateCustom")}</option>
                        )}
                        {templateOptions.map((option) => (
                          <option key={option.value} value={option.value}>{option.label}</option>
                        ))}
                      </select>
                    </label>
                  </div>
                  <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
                    <label className="flex items-center gap-3 text-sm">
                      <Switch
                        checked={profile.code_aware}
                        disabled={busy !== null}
                        onCheckedChange={(codeAware) => patchProfile(profile.id, { code_aware: codeAware })}
                      />
                      <span>{t("stylesPage.codeAware")}</span>
                    </label>
                    <Button variant="outline" disabled={busy !== null} onClick={() => void saveProfile(profile)}>
                      <FloppyDisk className="mr-2 h-4 w-4" />
                      {t("stylesPage.saveProfile")}
                    </Button>
                  </div>
                </article>
              );
            })}
          </section>

          <section className="space-y-4">
            <div className="space-y-1">
              <h2 className="text-lg font-semibold">{t("stylesPage.assignmentsTitle")}</h2>
              <p className="text-sm text-muted-foreground">{t("stylesPage.assignmentsDescription")}</p>
            </div>
            <Card className="p-5 md:p-6">
              <div className="grid gap-4 md:grid-cols-3">
                <label className="space-y-2 text-sm font-medium">
                  <span>{t("stylesPage.applicationLabel")}</span>
                  <select
                    aria-label={t("stylesPage.applicationLabel")}
                    value={assignment.application_id}
                    onChange={(event) => setAssignment({ ...assignment, application_id: event.target.value })}
                    className="h-10 w-full rounded-2xl border border-border bg-background px-4 text-sm"
                  >
                    <option value="">{t("stylesPage.chooseApplication")}</option>
                    {applicationOptions.map((application) => (
                      <option key={application.id} value={application.id}>{application.name}</option>
                    ))}
                    <option value={CUSTOM_APPLICATION}>{t("stylesPage.customApplication")}</option>
                  </select>
                </label>
                {assignment.application_id === CUSTOM_APPLICATION && (
                  <label className="space-y-2 text-sm font-medium">
                    <span>{t("stylesPage.customApplicationLabel")}</span>
                    <Input
                      aria-label={t("stylesPage.customApplicationLabel")}
                      value={assignment.custom_application_id}
                      onChange={(event) => setAssignment({ ...assignment, custom_application_id: event.target.value })}
                      placeholder={t("stylesPage.applicationPlaceholder")}
                    />
                  </label>
                )}
                <label className="space-y-2 text-sm font-medium">
                  <span>{t("stylesPage.windowTitleLabel")}</span>
                  <Input
                    aria-label={t("stylesPage.windowTitleLabel")}
                    value={assignment.title_contains}
                    onChange={(event) => setAssignment({ ...assignment, title_contains: event.target.value })}
                    placeholder={t("stylesPage.windowTitlePlaceholder")}
                  />
                </label>
                <label className="space-y-2 text-sm font-medium">
                  <span>{t("stylesPage.profileLabel")}</span>
                  <select
                    aria-label={t("stylesPage.profileLabel")}
                    value={assignment.profile_id}
                    onChange={(event) => setAssignment({ ...assignment, profile_id: event.target.value })}
                    className="h-10 w-full rounded-2xl border border-border bg-background px-4 text-sm"
                  >
                    <option value="">{t("stylesPage.chooseProfile")}</option>
                    {profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
                  </select>
                </label>
              </div>
              <Button className="mt-4" disabled={busy !== null} onClick={() => void addAssignment()}>
                <Plus className="mr-2 h-4 w-4" />
                {t("stylesPage.addAssignment")}
              </Button>
            </Card>

            {rules.length === 0 ? (
              <Card className="p-10 text-center text-sm text-muted-foreground">{t("stylesPage.emptyAssignments")}</Card>
            ) : rules.map((rule) => {
              const knownApplication = applicationOptions.find((application) => application.id === rule.application_id);
              const applicationChoice = knownApplication?.id ?? CUSTOM_APPLICATION;
              return (
              <article key={rule.id} className="rounded-3xl border border-border bg-card p-5 md:p-6">
                <h3 className="mb-4 font-semibold">{knownApplication?.name ?? rule.application_id}</h3>
                <div className="grid gap-4 md:grid-cols-3">
                  <label className="space-y-2 text-sm font-medium">
                    <span>{t("stylesPage.applicationLabel")}</span>
                    <select
                      aria-label={t("stylesPage.applicationLabel")}
                      value={applicationChoice}
                      onChange={(event) => {
                        patchRule(rule.id, {
                          application_id: event.target.value === CUSTOM_APPLICATION ? "" : event.target.value,
                        });
                      }}
                      className="h-10 w-full rounded-2xl border border-border bg-background px-4 text-sm"
                    >
                      {applicationOptions.map((application) => (
                        <option key={application.id} value={application.id}>{application.name}</option>
                      ))}
                      <option value={CUSTOM_APPLICATION}>{t("stylesPage.customApplication")}</option>
                    </select>
                  </label>
                  {applicationChoice === CUSTOM_APPLICATION && (
                    <label className="space-y-2 text-sm font-medium">
                      <span>{t("stylesPage.customApplicationLabel")}</span>
                      <Input
                        aria-label={t("stylesPage.customApplicationLabel")}
                        value={rule.application_id}
                        onChange={(event) => patchRule(rule.id, { application_id: event.target.value })}
                      />
                    </label>
                  )}
                  <label className="space-y-2 text-sm font-medium">
                    <span>{t("stylesPage.windowTitleLabel")}</span>
                    <Input
                      aria-label={t("stylesPage.windowTitleLabel")}
                      value={rule.title_contains ?? ""}
                      onChange={(event) => patchRule(rule.id, { title_contains: event.target.value.trim() || null })}
                    />
                  </label>
                  <label className="space-y-2 text-sm font-medium">
                    <span>{t("stylesPage.profileLabel")}</span>
                    <select
                      aria-label={t("stylesPage.profileLabel")}
                      value={rule.profile_id}
                      onChange={(event) => patchRule(rule.id, { profile_id: event.target.value })}
                      className="h-10 w-full rounded-2xl border border-border bg-background px-4 text-sm"
                    >
                      {profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
                    </select>
                  </label>
                </div>
                <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
                  <label className="flex items-center gap-3 text-sm">
                    <Switch
                      checked={rule.enabled}
                      disabled={busy !== null}
                      onCheckedChange={(enabled) => {
                        const updated = { ...rule, enabled };
                        patchRule(rule.id, { enabled });
                        void saveRule(updated, t("stylesPage.assignmentSaved"));
                      }}
                    />
                    <span>{t("stylesPage.enabled")}</span>
                  </label>
                  <div className="flex gap-2">
                    <Button variant="outline" disabled={busy !== null} onClick={() => void saveRule(rule, t("stylesPage.assignmentSaved"))}>
                      <FloppyDisk className="mr-2 h-4 w-4" />
                      {t("stylesPage.saveAssignment")}
                    </Button>
                    <Button variant="ghost" className="text-destructive" disabled={busy !== null} onClick={() => void deleteRule(rule)}>
                      <Trash className="mr-2 h-4 w-4" />
                      {t("stylesPage.deleteAssignment")}
                    </Button>
                  </div>
                </div>
              </article>
              );
            })}
          </section>
        </>
      )}
    </SettingsPageLayout>
  );
}
