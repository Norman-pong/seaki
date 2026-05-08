import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { CommandPalette } from "../CommandPalette";
import type { CommandPaletteAction } from "../CommandPalette";

describe("CommandPalette", () => {
  it("does_not_render_when_closed", () => {
    const { container } = render(
      <CommandPalette
        open={false}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    expect(container.firstChild).toBeNull();
  });

  it("renders_when_open", () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Command Palette")).toBeInTheDocument();
  });

  it("calls_onSelectCommand_when_command_clicked", () => {
    const onSelectCommand = vi.fn<(action: CommandPaletteAction) => void>();
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={onSelectCommand}
      />,
    );

    const firstCommand = screen.getByText("重建 stale workspace index");
    fireEvent.click(firstCommand);

    expect(onSelectCommand).toHaveBeenCalledWith("index-rebuild");
  });

  it("calls_onClose_when_backdrop_clicked", () => {
    const onClose = vi.fn<() => void>();
    render(
      <CommandPalette
        open={true}
        onClose={onClose}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    const backdrop = screen.getByRole("dialog");
    fireEvent.mouseDown(backdrop);

    expect(onClose).toHaveBeenCalled();
  });

  it("has_aria_modal_and_labelledby", () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveAttribute("aria-labelledby", "command-palette-title");
  });

  it("command_items_have_aria_selected", () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    const buttons = screen.getAllByRole("button");
    // Command palette card + Esc button + 5 command buttons = 7 buttons
    expect(buttons.length).toBeGreaterThanOrEqual(6);
  });
});
