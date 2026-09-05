import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/ui/confirm";
import type { WorkflowSettingsSnapshot } from "@/lib/tauri";
import { SnippetsPage } from "@/components/Home/SnippetsPage";

const {
  deleteSnippetMock,
  getSettingsMock,
  showErrorToastMock,
  upsertSnippetMock,
} = vi.hoisted(() => ({
  deleteSnippetMock: vi.fn(),
  getSettingsMock: vi.fn(),
  showErrorToastMock: vi.fn(),
  upsertSnippetMock: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  workflowCommands: {
    deleteVoiceSnippet: deleteSnippetMock,
    getSettings: getSettingsMock,
    upsertVoiceSnippet: upsertSnippetMock,
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
  profiles: [],
  application_rules: [],
  snippets: [
    {
      id: "meeting-note",
      spoken_trigger: "meeting note",
      template: "{{date}}: {{selection}}",
      enabled: true,
    },
    {
      id: "sign-off",
      spoken_trigger: "friendly sign off",
      template: "Thanks, Berny",
      enabled: true,
    },
  ],
};

function renderPage() {
  return render(
    <ConfirmProvider>
      <SnippetsPage />
    </ConfirmProvider>,
  );
}

describe("SnippetsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSettingsMock.mockResolvedValue(snapshot);
    upsertSnippetMock.mockResolvedValue(undefined);
    deleteSnippetMock.mockResolvedValue(undefined);
  });

  it("searches snippets and creates one without exposing an ID field", async () => {
    renderPage();
    expect(await screen.findByDisplayValue("meeting note")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("snippetsPage.searchPlaceholder"), {
      target: { value: "friendly" },
    });
    expect(screen.queryByDisplayValue("meeting note")).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("friendly sign off")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("snippetsPage.triggerPlaceholder"), {
      target: { value: "daily update" },
    });
    fireEvent.change(screen.getByPlaceholderText("snippetsPage.contentPlaceholder"), {
      target: { value: "Update for {{date}}" },
    });
    fireEvent.click(screen.getByRole("button", { name: "snippetsPage.add" }));

    await waitFor(() => expect(upsertSnippetMock).toHaveBeenCalledWith({
      id: "daily-update",
      spoken_trigger: "daily update",
      template: "Update for {{date}}",
      enabled: true,
    }));
  });

  it("saves edits, toggles availability, and deletes through typed commands", async () => {
    renderPage();
    const card = (await screen.findByDisplayValue("meeting note")).closest("article");
    expect(card).not.toBeNull();
    const scoped = within(card as HTMLElement);

    fireEvent.change(scoped.getByLabelText("snippetsPage.triggerLabel"), {
      target: { value: "meeting summary" },
    });
    fireEvent.click(scoped.getByRole("button", { name: "snippetsPage.save" }));
    await waitFor(() => expect(upsertSnippetMock).toHaveBeenCalledWith(expect.objectContaining({
      id: "meeting-note",
      spoken_trigger: "meeting summary",
    })));

    upsertSnippetMock.mockClear();
    fireEvent.click(scoped.getByRole("switch", { name: "snippetsPage.enabled" }));
    await waitFor(() => expect(upsertSnippetMock).toHaveBeenCalledWith(expect.objectContaining({
      id: "meeting-note",
      enabled: false,
    })));

    fireEvent.click(scoped.getByRole("button", { name: "snippetsPage.delete" }));
    fireEvent.click(await screen.findByRole("button", { name: "snippetsPage.deleteConfirm" }));
    await waitFor(() => expect(deleteSnippetMock).toHaveBeenCalledWith("meeting-note"));
  });

  it("keeps drafts in other rows when one snippet is saved", async () => {
    renderPage();
    const meetingInput = await screen.findByDisplayValue("meeting note");
    const signOffInput = screen.getByDisplayValue("friendly sign off");

    fireEvent.change(signOffInput, { target: { value: "warm sign off" } });
    fireEvent.click(within(meetingInput.closest("article") as HTMLElement)
      .getByRole("button", { name: "snippetsPage.save" }));

    await waitFor(() => expect(upsertSnippetMock).toHaveBeenCalled());
    expect(screen.getByDisplayValue("warm sign off")).toBeInTheDocument();
    expect(getSettingsMock).toHaveBeenCalledTimes(1);
  });

  it("reports backend load errors", async () => {
    getSettingsMock.mockRejectedValueOnce("workflow unavailable");
    renderPage();

    await waitFor(() => {
      expect(showErrorToastMock).toHaveBeenCalledWith("snippetsPage.loadError");
      expect(screen.getByRole("alert")).toHaveTextContent("snippetsPage.loadError");
    });
  });
});
