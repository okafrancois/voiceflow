import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/ui/confirm";
import type { WorkflowSettingsSnapshot } from "@/lib/tauri";
import { StylesPage } from "@/components/Home/StylesPage";

const {
  createProfileMock,
  deleteRuleMock,
  getCustomTemplatesMock,
  getPlatformMock,
  getSettingsMock,
  showErrorToastMock,
  updateProfileMock,
  upsertRuleMock,
} = vi.hoisted(() => ({
  createProfileMock: vi.fn(),
  deleteRuleMock: vi.fn(),
  getCustomTemplatesMock: vi.fn(),
  getPlatformMock: vi.fn(),
  getSettingsMock: vi.fn(),
  showErrorToastMock: vi.fn(),
  updateProfileMock: vi.fn(),
  upsertRuleMock: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  modelCommands: {
    getPolishCustomTemplates: getCustomTemplatesMock,
  },
  systemCommands: {
    getPlatform: getPlatformMock,
  },
  workflowCommands: {
    createProfile: createProfileMock,
    deleteApplicationRule: deleteRuleMock,
    getSettings: getSettingsMock,
    updateProfile: updateProfileMock,
    upsertApplicationRule: upsertRuleMock,
  },
}));

vi.mock("@/lib/toast", () => ({
  showErrorToast: showErrorToastMock,
  showToast: vi.fn(),
}));

const translate = (key: string) => key;

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: translate }),
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
      name: "Everyday",
      hotkey: "Cmd+Slash",
      trigger_mode: "hold",
      language: "en",
      polish_template_id: "chat",
      translation_target: null,
      output_action: "insert",
      code_aware: false,
      protected: true,
    },
    {
      id: "formal",
      name: "Formal",
      hotkey: "Cmd+2",
      trigger_mode: "toggle",
      language: null,
      polish_template_id: "formal",
      translation_target: null,
      output_action: "copy",
      code_aware: false,
      protected: false,
    },
    {
      id: "client",
      name: "Client voice",
      hotkey: "Cmd+3",
      trigger_mode: "hold",
      language: null,
      polish_template_id: "custom-client",
      translation_target: null,
      output_action: "insert",
      code_aware: false,
      protected: false,
    },
  ],
  application_rules: [
    {
      id: "mail",
      application_id: "com.apple.mail",
      title_contains: null,
      profile_id: "formal",
      enabled: true,
    },
  ],
  snippets: [],
};

function renderPage() {
  return render(
    <ConfirmProvider>
      <StylesPage />
    </ConfirmProvider>,
  );
}

describe("StylesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSettingsMock.mockResolvedValue(snapshot);
    getCustomTemplatesMock.mockResolvedValue([
      { id: "custom-client", name: "Client voice", system_prompt: "Use the client voice." },
    ]);
    getPlatformMock.mockResolvedValue("macos");
    updateProfileMock.mockResolvedValue(undefined);
    createProfileMock.mockResolvedValue(undefined);
    upsertRuleMock.mockResolvedValue(undefined);
    deleteRuleMock.mockResolvedValue(undefined);
  });

  it("creates a named app-only style without allocating a global shortcut", async () => {
    renderPage();
    await screen.findByRole("heading", { name: "stylesPage.createTitle" });

    fireEvent.change(screen.getByLabelText("stylesPage.createNameLabel"), {
      target: { value: "Email replies" },
    });
    fireEvent.change(screen.getByLabelText("stylesPage.createTemplateLabel"), {
      target: { value: "concise" },
    });
    fireEvent.click(screen.getByRole("button", { name: "stylesPage.createStyle" }));

    await waitFor(() => expect(createProfileMock).toHaveBeenCalledWith({
      id: "email-replies",
      name: "Email replies",
      hotkey: "",
      trigger_mode: "hold",
      language: null,
      polish_template_id: "concise",
      translation_target: null,
      output_action: "insert",
      code_aware: false,
      protected: false,
    }));
    expect(await screen.findByRole("heading", { name: "Email replies" })).toBeInTheDocument();
    expect(getSettingsMock).toHaveBeenCalledTimes(1);
  });

  it("updates style fields while preserving the complete profile contract", async () => {
    renderPage();
    const card = (await screen.findAllByText("Everyday"))[0].closest("article");
    expect(card).not.toBeNull();
    const scoped = within(card as HTMLElement);

    fireEvent.change(scoped.getByLabelText("stylesPage.nameLabel"), {
      target: { value: "Messages" },
    });
    fireEvent.change(scoped.getByLabelText("stylesPage.templateLabel"), {
      target: { value: "concise" },
    });
    fireEvent.click(scoped.getByRole("switch", { name: "stylesPage.codeAware" }));
    fireEvent.click(scoped.getByRole("button", { name: "stylesPage.saveProfile" }));

    await waitFor(() => expect(updateProfileMock).toHaveBeenCalledWith({
      ...snapshot.profiles[0],
      name: "Messages",
      polish_template_id: "concise",
      code_aware: true,
    }));
  });

  it("creates, edits, toggles, and deletes application assignments", async () => {
    renderPage();
    await screen.findByRole("heading", { name: "Mail" });
    expect(screen.queryByText("com.apple.mail")).not.toBeInTheDocument();

    fireEvent.change(screen.getAllByLabelText("stylesPage.applicationLabel")[0], {
      target: { value: "com.tinyspeck.slackmacgap" },
    });
    fireEvent.change(screen.getByPlaceholderText("stylesPage.windowTitlePlaceholder"), {
      target: { value: "Project" },
    });
    fireEvent.change(screen.getAllByLabelText("stylesPage.profileLabel")[0], {
      target: { value: "dictate" },
    });
    fireEvent.click(screen.getByRole("button", { name: "stylesPage.addAssignment" }));
    await waitFor(() => expect(upsertRuleMock).toHaveBeenCalledWith({
      id: "com-tinyspeck-slackmacgap",
      application_id: "com.tinyspeck.slackmacgap",
      title_contains: "Project",
      profile_id: "dictate",
      enabled: true,
    }));

    upsertRuleMock.mockClear();
    const card = screen.getByRole("heading", { name: "Mail" }).closest("article");
    expect(card).not.toBeNull();
    const scoped = within(card as HTMLElement);
    fireEvent.click(scoped.getByRole("switch", { name: "stylesPage.enabled" }));
    await waitFor(() => expect(upsertRuleMock).toHaveBeenCalledWith({
      ...snapshot.application_rules[0],
      enabled: false,
    }));

    fireEvent.click(scoped.getByRole("button", { name: "stylesPage.deleteAssignment" }));
    fireEvent.click(await screen.findByRole("button", { name: "stylesPage.deleteConfirm" }));
    await waitFor(() => expect(deleteRuleMock).toHaveBeenCalledWith("mail"));
  });

  it("shows a custom template name and preserves its ID on save", async () => {
    renderPage();
    const card = (await screen.findByRole("heading", { name: "Client voice" })).closest("article");
    expect(card).not.toBeNull();
    const scoped = within(card as HTMLElement);

    expect(scoped.getByLabelText("stylesPage.templateLabel")).toHaveDisplayValue("Client voice");
    fireEvent.click(scoped.getByRole("button", { name: "stylesPage.saveProfile" }));

    await waitFor(() => expect(updateProfileMock).toHaveBeenCalledWith(snapshot.profiles[2]));
  });

  it("reports backend load errors", async () => {
    getSettingsMock.mockRejectedValueOnce("workflow unavailable");
    renderPage();

    await waitFor(() => {
      expect(showErrorToastMock).toHaveBeenCalledWith("stylesPage.loadError");
      expect(screen.getByRole("alert")).toHaveTextContent("stylesPage.loadError");
    });
  });
});
