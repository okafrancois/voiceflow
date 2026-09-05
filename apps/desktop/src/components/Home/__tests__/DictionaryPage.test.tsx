import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DictionaryPage } from "@/components/Home/DictionaryPage";

const {
  getAutoEntriesMock,
  getCustomEntriesMock,
  translateMock,
  updateCustomEntryMock,
} = vi.hoisted(() => ({
  getAutoEntriesMock: vi.fn(),
  getCustomEntriesMock: vi.fn(),
  translateMock: (key: string) => key,
  updateCustomEntryMock: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  dictionaryCommands: {
    addCustomEntry: vi.fn(),
    deleteAutoEntry: vi.fn(),
    deleteCustomEntry: vi.fn(),
    getAutoEntries: getAutoEntriesMock,
    getCustomEntries: getCustomEntriesMock,
    importCustomCsv: vi.fn(),
    updateCustomEntry: updateCustomEntryMock,
  },
  events: {
    onCorrectionLearned: vi.fn(async () => () => undefined),
    onSettingsChanged: vi.fn(async () => () => undefined),
  },
}));

vi.mock("@/lib/toast", () => ({
  showErrorToast: vi.fn(),
  showToast: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: translateMock }),
}));

describe("DictionaryPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getAutoEntriesMock.mockResolvedValue([]);
    getCustomEntriesMock.mockResolvedValue([
      {
        term: "Claude",
        aliases: ["Cloud", "Clawed"],
        frequency: 1,
        first_seen_at_ms: 0,
        last_seen_at_ms: 0,
        source: "custom_dictionary",
      },
    ]);
    updateCustomEntryMock.mockResolvedValue(undefined);
  });

  it("edits explicit heard-as aliases for a manual dictionary entry", async () => {
    render(<MemoryRouter><DictionaryPage /></MemoryRouter>);
    await waitFor(() => expect(getCustomEntriesMock).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByText("dictionary.loading")).not.toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "dictionary.tabs.manual" }));
    const row = (await screen.findByText("Claude")).closest("div.group");
    expect(row).not.toBeNull();
    const scoped = within(row as HTMLElement);

    const aliases = scoped.getByLabelText("dictionary.form.aliases");
    expect(aliases).toHaveValue("Cloud, Clawed");
    fireEvent.change(aliases, { target: { value: "Crowd, cloud" } });
    fireEvent.click(scoped.getByRole("button", { name: "dictionary.actions.saveAliases" }));

    await waitFor(() => expect(updateCustomEntryMock).toHaveBeenCalledWith(
      "Claude",
      ["Crowd", "cloud"],
    ));
  });
});
