import * as Dialog from "@radix-ui/react-dialog";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Select } from "../select";

function renderSelectInsideModal(onChange = vi.fn()) {
  render(
    <Dialog.Root open>
      <Dialog.Portal>
        <Dialog.Overlay />
        <Dialog.Content>
          <Dialog.Title>Settings</Dialog.Title>
          <Dialog.Description>Settings modal</Dialog.Description>
          <Select
            value="alpha"
            onChange={onChange}
            options={[
              { value: "alpha", label: "Alpha" },
              { value: "beta", label: "Beta" },
            ]}
          />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>,
  );

  return onChange;
}

function renderSearchableSelectInsideModal() {
  render(
    <Dialog.Root open>
      <Dialog.Portal>
        <Dialog.Overlay />
        <Dialog.Content>
          <Dialog.Title>Language</Dialog.Title>
          <Dialog.Description>Choose the transcription language</Dialog.Description>
          <Select
            value="language-0"
            onChange={vi.fn()}
            options={Array.from({ length: 10 }, (_, index) => ({
              value: `language-${index}`,
              label: `Language ${index}`,
            }))}
          />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>,
  );
}

describe("Select", () => {
  it("keeps modal dialog options pointer-enabled", () => {
    renderSelectInsideModal();

    fireEvent.click(screen.getByRole("combobox"));

    expect(screen.getByRole("listbox")).toHaveStyle({ pointerEvents: "auto" });
  });

  it("selects an option rendered from a modal dialog", () => {
    const onChange = renderSelectInsideModal();

    fireEvent.click(screen.getByRole("combobox"));
    fireEvent.click(screen.getByRole("button", { name: /beta/i }));

    expect(onChange).toHaveBeenCalledWith({ target: { value: "beta" } });
  });

  it("keeps the search input focusable and usable inside a modal dialog", async () => {
    renderSearchableSelectInsideModal();

    fireEvent.click(screen.getByRole("combobox"));
    const searchInput = screen.getByRole("textbox");
    await waitFor(() => expect(searchInput).toHaveFocus());
    fireEvent.change(searchInput, { target: { value: "Language 9" } });

    expect(screen.getByRole("button", { name: "Language 9" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Language 1" })).not.toBeInTheDocument();
  });
});
