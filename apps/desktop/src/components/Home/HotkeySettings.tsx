import { useState, useEffect } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { useTranslation } from "react-i18next";
import { analytics } from "@/lib/analytics";
import { AnalyticsEvents } from "@/lib/events";
import { useSettingsContext } from "@/contexts/SettingsContext";
import { HotkeyInput } from "@/components/ui/hotkey-input";
import { MultiSwitch } from "@/components/ui/multi-switch";
import { SettingsPageLayout } from "./SettingsPageLayout";
import {
  hotkeyCommands,
  modelCommands,
  type ShortcutProfile,
  type ShortcutTriggerMode,
  type PolishTemplate,
  type CustomPolishTemplate,
} from "@/lib/tauri";
import { WarningCircle } from "@phosphor-icons/react";

const RECORDING_MODES = [
  { value: "hold", label: "Hold" },
  { value: "toggle", label: "Toggle" },
  { value: "double_tap", label: "Double Tap" },
] as const;

interface ProfileSectionProps {
  profileKey: string;
  profile?: ShortcutProfile;
  templates: (PolishTemplate | CustomPolishTemplate)[];
  canChangeTemplate: boolean;
  allowNullTemplate: boolean;
  polishAvailable: boolean;
  onUpdate: (
    hotkey: string,
    templateId: string | null,
    triggerMode: ShortcutTriggerMode,
  ) => void;
  testId?: string;
}

function ProfileSection({
  profileKey,
  profile,
  templates,
  canChangeTemplate,
  allowNullTemplate,
  polishAvailable,
  onUpdate,
  testId,
}: ProfileSectionProps) {
  const { t } = useTranslation();
  const templateId = profile?.action?.Record?.polish_template_id ?? null;
  const triggerMode = profile?.trigger_mode ?? "hold";

  const templateOptions = [
    ...(allowNullTemplate ? [{ value: "", label: t("hotkey.noPolish", "No Polish") }] : []),
    ...templates.map((tpl) => ({ value: tpl.id, label: tpl.name })),
  ];
  const recordingModes = RECORDING_MODES.map((option) => ({
    value: option.value,
    label:
      option.value === "hold"
        ? t("hotkey.recording.modeHold")
        : option.value === "toggle"
          ? t("hotkey.recording.modeToggle")
          : t("hotkey.recording.modeDoubleTap"),
  }));

  return (
    <div className="space-y-4" data-testid={testId}>
      <div className="space-y-2">
        <Label>{t("hotkey.hotkey", "Hotkey")}</Label>
        <HotkeyInput
          profileKey={profileKey}
          value={profile?.hotkey || ""}
          onChange={(hotkey) => onUpdate(hotkey, templateId, triggerMode)}
          placeholder={t("hotkey.recording.pressKeys")}
          className="w-auto"
        />
      </div>

      <div className="space-y-4">
        <div className="text-sm font-medium">{t("hotkey.recording.modeTitle")}</div>
        <MultiSwitch
          options={recordingModes}
          value={triggerMode}
          onChange={(value) =>
            onUpdate(profile?.hotkey || "", templateId, value as ShortcutTriggerMode)
          }
        />
      </div>

      {canChangeTemplate && (
        <div className="space-y-2">
          <Label>{t("hotkey.template", "Polish Template")}</Label>
          <Select
            aria-label={t("hotkey.template", "Polish Template")}
            value={templateId || ""}
            onChange={(e) =>
              onUpdate(
                profile?.hotkey || "",
                allowNullTemplate ? (e.target.value || null) : e.target.value,
                triggerMode,
              )
            }
            options={templateOptions}
            placeholder={t("hotkey.selectTemplate", "Select template")}
          />
          {!polishAvailable && (
            <p className="text-xs text-amber-500 flex items-center gap-1">
              <WarningCircle className="h-3 w-3" />
              {t("polish.unavailableHint")}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

interface HotkeySettingsProps {
  variant?: "page" | "modal";
}

export function HotkeySettings({ variant = "page" }: HotkeySettingsProps = {}) {
  const { t } = useTranslation();
  const { settings, polishAvailable } = useSettingsContext();
  const [templates, setTemplates] = useState<(PolishTemplate | CustomPolishTemplate)[]>([]);

  useEffect(() => {
    loadTemplates();
  }, []);

  const loadTemplates = async () => {
    try {
      const [builtIn, custom] = await Promise.all([
        modelCommands.getPolishTemplates(),
        modelCommands.getPolishCustomTemplates(),
      ]);
      setTemplates([...builtIn, ...custom]);
    } catch (err) {
      console.error("Failed to load templates:", err);
    }
  };

  if (!settings) return null;

  const profiles = settings.shortcut_profiles;

  const handleUpdateDictate = async (
    hotkey: string,
    templateId: string | null,
    triggerMode: ShortcutTriggerMode,
  ) => {
    analytics.track(AnalyticsEvents.SETTING_CHANGED, {
      setting: "dictate_profile",
      value: `${hotkey}:${templateId ?? "none"}`,
    });
    await hotkeyCommands.updateProfile("dictate", {
      hotkey,
      trigger_mode: triggerMode,
      action: { Record: { polish_template_id: templateId } },
    });
  };

  return (
    <SettingsPageLayout
      title={t("hotkey.title")}
      description={t("hotkey.description")}
      testId="hotkey-page"
      variant={variant}
      showHeader={variant === "page"}
    >
      <Card>
        <CardHeader>
          <CardTitle>{t("hotkey.profiles.dictate", "Dictate")}</CardTitle>
          <CardDescription>{t("hotkey.profiles.dictateDesc", "Voice-to-text with optional polish")}</CardDescription>
        </CardHeader>
        <CardContent>
          <ProfileSection
            profileKey="dictate"
            profile={profiles?.dictate}
            templates={templates}
            canChangeTemplate={true}
            allowNullTemplate={true}
            polishAvailable={polishAvailable}
            onUpdate={handleUpdateDictate}
            testId="profile-dictate"
          />
        </CardContent>
      </Card>

    </SettingsPageLayout>
  );
}
