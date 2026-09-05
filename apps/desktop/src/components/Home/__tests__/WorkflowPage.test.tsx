import { MemoryRouter } from "react-router-dom";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/ui/confirm";
import type { WorkflowSettingsSnapshot } from "@/lib/tauri";
import { WorkflowPage } from "../WorkflowPage";

const {
  getSettingsMock,
  createProfileMock,
  updateProfileMock,
  deleteProfileMock,
  upsertRuleMock,
  deleteRuleMock,
  upsertSnippetMock,
  deleteSnippetMock,
  runVoiceActionMock,
  replacePreviewMock,
  runQuickControlMock,
  showErrorToastMock,
  translateMock,
} = vi.hoisted(() => ({
  getSettingsMock: vi.fn(),
  createProfileMock: vi.fn(),
  updateProfileMock: vi.fn(),
  deleteProfileMock: vi.fn(),
  upsertRuleMock: vi.fn(),
  deleteRuleMock: vi.fn(),
  upsertSnippetMock: vi.fn(),
  deleteSnippetMock: vi.fn(),
  runVoiceActionMock: vi.fn(),
  replacePreviewMock: vi.fn(),
  runQuickControlMock: vi.fn(),
  showErrorToastMock: vi.fn(),
  translateMock: (key: string) => key,
}));

vi.mock("@/lib/tauri", () => ({
  workflowCommands: {
    getSettings: getSettingsMock,
    captureContext: vi.fn(),
    setContextCapture: vi.fn(),
    createProfile: createProfileMock,
    updateProfile: updateProfileMock,
    deleteProfile: deleteProfileMock,
    upsertApplicationRule: upsertRuleMock,
    deleteApplicationRule: deleteRuleMock,
    upsertVoiceSnippet: upsertSnippetMock,
    deleteVoiceSnippet: deleteSnippetMock,
    expandVoiceSnippet: vi.fn(),
    runVoiceAction: runVoiceActionMock,
    replaceVoiceActionPreview: replacePreviewMock,
    runQuickControl: runQuickControlMock,
  },
}));

vi.mock("@/lib/toast", () => ({
  showToast: vi.fn(),
  showErrorToast: showErrorToastMock,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: translateMock,
  }),
}));

const snapshot: WorkflowSettingsSnapshot = {
  context_capture: {
    application_metadata: true,
    focused_field: true,
    selected_text: true,
    clipboard: false,
    ocr_fallback: false,
  },
  profiles: [
    {
      id: "dictate",
      name: "Dictate",
      hotkey: "Cmd+Slash",
      trigger_mode: "hold",
      language: null,
      polish_template_id: null,
      translation_target: null,
      output_action: "insert",
      code_aware: false,
      protected: true,
    },
  ],
  application_rules: [
    {
      id: "editor",
      application_id: "com.example.Editor",
      title_contains: null,
      profile_id: "dictate",
      enabled: true,
    },
  ],
  snippets: [
    {
      id: "meeting-note",
      spoken_trigger: "meeting note",
      template: "{{date}}: {{selection}}",
      enabled: true,
    },
  ],
};

function renderPage() {
  return render(
    <MemoryRouter><ConfirmProvider>
      <WorkflowPage />
    </ConfirmProvider></MemoryRouter>,
  );
}

describe("WorkflowPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSettingsMock.mockResolvedValue(snapshot);
    createProfileMock.mockResolvedValue(undefined);
    updateProfileMock.mockResolvedValue(undefined);
    deleteProfileMock.mockResolvedValue(undefined);
    upsertRuleMock.mockResolvedValue(undefined);
    deleteRuleMock.mockResolvedValue(undefined);
    upsertSnippetMock.mockResolvedValue(undefined);
    deleteSnippetMock.mockResolvedValue(undefined);
    runVoiceActionMock.mockResolvedValue({
      kind: "shorten",
      source_text: "Long selected text",
      result_text: "Short text",
      translation_target: null,
      output_action: "preview",
    });
    replacePreviewMock.mockResolvedValue({
      kind: "shorten",
      source_text: "Long selected text",
      result_text: "Short text",
      translation_target: null,
      output_action: "preview",
    });
    runQuickControlMock.mockResolvedValue({ action: "copy_final", text: "Short text" });
  });

  it("creates more than one named profile through the canonical list", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "workflow.tabs.profiles" }));

    const addProfile = async (id: string, name: string, hotkey: string) => {
      const addButton = screen.getByRole("button", { name: "workflow.actions.add" });
      await waitFor(() => expect(addButton).toBeEnabled());
      fireEvent.change(screen.getByLabelText("workflow.profiles.fields.id"), {
        target: { value: id },
      });
      const names = screen.getAllByLabelText("workflow.profiles.fields.name");
      const hotkeys = screen.getAllByLabelText("workflow.profiles.fields.hotkey");
      fireEvent.change(names[names.length - 1], { target: { value: name } });
      fireEvent.change(hotkeys[hotkeys.length - 1], { target: { value: hotkey } });
      fireEvent.click(addButton);
      await waitFor(() => expect(createProfileMock).toHaveBeenCalledTimes(1));
    };

    await addProfile("code", "Code", "Cmd+1");
    createProfileMock.mockClear();
    await addProfile("reply", "Reply", "Cmd+2");

    expect(createProfileMock).toHaveBeenCalledWith(expect.objectContaining({
      id: "reply",
      name: "Reply",
      hotkey: "Cmd+2",
    }));
  });

  it("updates application rules and voice snippets through backend commands", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "workflow.tabs.rules" }));
    const ruleSection = screen.getByText("com.example.Editor").closest("div.rounded-2xl");
    expect(ruleSection).not.toBeNull();
    fireEvent.click(within(ruleSection as HTMLElement).getByRole("button", { name: "workflow.actions.save" }));
    await waitFor(() => expect(upsertRuleMock).toHaveBeenCalledWith(snapshot.application_rules[0]));

    fireEvent.click(screen.getByRole("tab", { name: "workflow.tabs.snippets" }));
    const snippetSection = screen.getByText("meeting note").closest("div.rounded-2xl");
    expect(snippetSection).not.toBeNull();
    fireEvent.click(within(snippetSection as HTMLElement).getByRole("button", { name: "workflow.actions.save" }));
    await waitFor(() => expect(upsertSnippetMock).toHaveBeenCalledWith(snapshot.snippets[0]));
  });

  it("previews a selected-text action and invokes explicit replacement", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "workflow.tabs.actions" }));
    fireEvent.change(screen.getByLabelText("workflow.voiceActions.source"), {
      target: { value: "Long selected text" },
    });
    const runButton = screen.getByRole("button", { name: "workflow.voiceActions.run" });
    await waitFor(() => expect(runButton).toBeEnabled());
    fireEvent.click(runButton);

    expect(await screen.findByText("Short text")).toBeInTheDocument();
    expect(runVoiceActionMock).toHaveBeenCalledWith(expect.objectContaining({
      kind: "shorten",
      selected_text: "Long selected text",
      output_action: "preview",
    }));
    fireEvent.click(screen.getByRole("button", { name: "workflow.voiceActions.replace" }));
    await waitFor(() => expect(replacePreviewMock).toHaveBeenCalled());
  });

  it("keeps unavailable quick controls as backend errors", async () => {
    runQuickControlMock.mockRejectedValueOnce("No previous delivery is available");
    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "workflow.tabs.actions" }));
    const copyButton = screen.getByRole("button", { name: "workflow.quick.copy_final" });
    await waitFor(() => expect(copyButton).toBeEnabled());
    fireEvent.click(copyButton);

    await waitFor(() => {
      expect(runQuickControlMock).toHaveBeenCalledWith("copy_final");
      expect(showErrorToastMock).toHaveBeenCalledWith("No previous delivery is available");
    });
  });
});
