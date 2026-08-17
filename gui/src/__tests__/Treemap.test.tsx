import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Treemap } from "../components/Treemap";
import type { FileNode } from "../domain";

function makeNode(name: string, size: number, isDir = false): FileNode {
  return {
    path: `/root/${name}`,
    name,
    size,
    modified: "2024-01-01T00:00:00Z",
    node_type: "Other",
    is_dir: isDir,
    children: [],
  };
}

function makeDir(name: string, children: FileNode[]): FileNode {
  const size = children.reduce((s, c) => s + c.size, 0);
  return {
    path: `/root/${name}`,
    name,
    size,
    modified: "2024-01-01T00:00:00Z",
    node_type: "Other",
    is_dir: true,
    children,
  };
}

describe("Treemap", () => {
  it("renders rectangles for each child node", () => {
    const node = makeDir("root", [
      makeNode("a.txt", 100),
      makeNode("b.txt", 200),
      makeNode("c.txt", 300),
    ]);
    render(<Treemap node={node} onDrill={() => {}} parentSize={600} />);

    expect(screen.getByTestId("treemap")).toBeInTheDocument();
    expect(screen.getByTestId("treemap-cell-a.txt")).toBeInTheDocument();
    expect(screen.getByTestId("treemap-cell-b.txt")).toBeInTheDocument();
    expect(screen.getByTestId("treemap-cell-c.txt")).toBeInTheDocument();
  });

  it("shows tooltip on hover", async () => {
    const node = makeDir("root", [makeNode("file.txt", 500)]);
    render(<Treemap node={node} onDrill={() => {}} parentSize={500} />);

    const cell = screen.getByTestId("treemap-cell-file.txt");
    fireEvent.mouseEnter(cell, { clientX: 100, clientY: 100 });

    const tooltip = screen.getByTestId("treemap-tooltip");
    expect(tooltip).toBeInTheDocument();
    expect(tooltip).toHaveTextContent("file.txt");
    expect(tooltip).toHaveTextContent("500 B");
    expect(tooltip).toHaveTextContent("100.0%");
  });

  it("calls onDrill when clicking a directory cell", () => {
    const child = makeDir("subdir", [makeNode("x.txt", 10)]);
    const node = makeDir("root", [child]);
    const onDrill = vi.fn();

    render(<Treemap node={node} onDrill={onDrill} parentSize={10} />);
    fireEvent.click(screen.getByTestId("treemap-cell-subdir"));

    expect(onDrill).toHaveBeenCalledWith(child);
  });

  it("does not call onDrill when clicking a file cell", () => {
    const node = makeDir("root", [makeNode("file.txt", 100)]);
    const onDrill = vi.fn();

    render(<Treemap node={node} onDrill={onDrill} parentSize={100} />);
    fireEvent.click(screen.getByTestId("treemap-cell-file.txt"));

    expect(onDrill).not.toHaveBeenCalled();
  });

  it("handles empty directory", () => {
    const node = makeDir("empty", []);
    render(<Treemap node={node} onDrill={() => {}} parentSize={0} />);
    expect(screen.getByTestId("treemap")).toBeInTheDocument();
  });
});
