import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { CommandPalette, type CommandPaletteAction } from "../CommandPalette";

describe("CommandPalette", () => {
  it("does_not_render_when_closed", () => {
    render(
      <CommandPalette
        open={false}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    expect(screen.queryByTestId("command-palette-card")).not.toBeInTheDocument();
  });

  it("renders_when_open", () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("command-palette-card")).toBeInTheDocument();
    expect(screen.getByText("Command Palette")).toBeInTheDocument();
  });

  it("renders_all_commands_by_default", () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("command-row-index-rebuild")).toBeInTheDocument();
    expect(screen.getByTestId("command-row-approval-review")).toBeInTheDocument();
    expect(screen.getByTestId("command-row-attach-source")).toBeInTheDocument();
    expect(screen.getByTestId("command-row-pipe-dry-run")).toBeInTheDocument();
    expect(screen.getByTestId("command-row-wiki-search")).toBeInTheDocument();
  });

  it("filters_commands_by_query", () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "approval" },
    });

    expect(screen.queryByTestId("command-row-approval-review")).toBeInTheDocument();
    expect(screen.queryByTestId("command-row-index-rebuild")).not.toBeInTheDocument();
  });

  it("selects_command_on_click", () => {
    const onSelectCommand = vi.fn<(action: CommandPaletteAction) => void>();
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={onSelectCommand}
      />,
    );

    fireEvent.click(screen.getByTestId("command-row-approval-review"));
    expect(onSelectCommand).toHaveBeenCalledWith("approval-review");
  });

  it("closes_on_esc_key", () => {
    const onClose = vi.fn<() => void>();
    render(
      <CommandPalette
        open={true}
        onClose={onClose}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    fireEvent.keyDown(screen.getByTestId("command-palette-card"), {
      key: "Escape",
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("navigates_with_arrow_keys", () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    const card = screen.getByTestId("command-palette-card");

    expect(screen.getByTestId("command-row-index-rebuild")).toHaveAttribute("data-selected", "true");

    fireEvent.keyDown(card, { key: "ArrowDown" });
    expect(screen.getByTestId("command-row-approval-review")).toHaveAttribute("data-selected", "true");

    fireEvent.keyDown(card, { key: "ArrowUp" });
    expect(screen.getByTestId("command-row-index-rebuild")).toHaveAttribute("data-selected", "true");
  });

  it("executes_selected_command_on_enter", () => {
    const onSelectCommand = vi.fn<(action: CommandPaletteAction) => void>();
    const onClose = vi.fn<() => void>();
    render(
      <CommandPalette
        open={true}
        onClose={onClose}
        onSelectCommand={onSelectCommand}
      />,
    );

    fireEvent.keyDown(screen.getByTestId("command-palette-card"), {
      key: "Enter",
    });
    expect(onSelectCommand).toHaveBeenCalledWith("index-rebuild");
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("auto_focuses_input_when_opened", async () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("command-palette-input")).toHaveFocus();
    });
  });

  it("shows_no_results_when_query_matches_nothing", () => {
    render(
      <CommandPalette
        open={true}
        onClose={vi.fn<() => void>()}
        onSelectCommand={vi.fn<() => void>()}
      />,
    );

    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "xyz-nonexistent" },
    });

    expect(screen.getByText("无匹配命令")).toBeInTheDocument();
  });
});
