import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ContextMenu } from "../components/ContextMenu";

describe("ContextMenu", () => {
  const defaultProps = {
    x: 100,
    y: 200,
    visible: true,
    onOpenExplorer: vi.fn(),
    onCopyPath: vi.fn(),
    onCopySize: vi.fn(),
    onDelete: vi.fn(),
    onClose: vi.fn(),
  };

  it("renders when visible", () => {
    render(<ContextMenu {...defaultProps} />);
    expect(screen.getByTestId("context-menu")).toBeInTheDocument();
  });

  it("does not render when not visible", () => {
    render(<ContextMenu {...defaultProps} visible={false} />);
    expect(screen.queryByTestId("context-menu")).not.toBeInTheDocument();
  });

  it("calls onDelete when trash button clicked", () => {
    render(<ContextMenu {...defaultProps} />);
    fireEvent.click(screen.getByText("Move to trash"));
    expect(defaultProps.onDelete).toHaveBeenCalled();
  });

  it("calls onCopyPath when copy path clicked", () => {
    render(<ContextMenu {...defaultProps} />);
    fireEvent.click(screen.getByText("Copy path"));
    expect(defaultProps.onCopyPath).toHaveBeenCalled();
  });

  it("calls onClose when overlay clicked", () => {
    render(<ContextMenu {...defaultProps} />);
    fireEvent.click(document.querySelector(".context-menu-overlay")!);
    expect(defaultProps.onClose).toHaveBeenCalled();
  });
});
