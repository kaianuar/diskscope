import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Table } from "../components/Table";
import type { FileNode } from "../domain";

function makeNode(
  name: string,
  size: number,
  type: FileNode["node_type"] = "Other"
): FileNode {
  return {
    path: `/root/${name}`,
    name,
    size,
    modified: "2024-01-01T00:00:00Z",
    node_type: type,
    is_dir: false,
    children: [],
  };
}

describe("Table", () => {
  const nodes: FileNode[] = [
    makeNode("big.txt", 1000, "Document"),
    makeNode("small.rs", 50, "Code"),
    makeNode("image.png", 500, "Image"),
  ];

  it("renders all rows", () => {
    render(
      <Table
        nodes={nodes}
        selectedIndex={-1}
        onSelect={() => {}}
        onActivate={() => {}}
        onContextMenu={() => {}}
      />
    );
    expect(screen.getByTestId("table-row-big.txt")).toBeInTheDocument();
    expect(screen.getByTestId("table-row-small.rs")).toBeInTheDocument();
    expect(screen.getByTestId("table-row-image.png")).toBeInTheDocument();
  });

  it("sorts by size descending by default", () => {
    render(
      <Table
        nodes={nodes}
        selectedIndex={-1}
        onSelect={() => {}}
        onActivate={() => {}}
        onContextMenu={() => {}}
      />
    );
    const rows = screen.getAllByTestId(/^table-row-/);
    expect(rows[0]).toHaveTextContent("big.txt");
    expect(rows[1]).toHaveTextContent("image.png");
    expect(rows[2]).toHaveTextContent("small.rs");
  });

  it("toggles sort when clicking column header", () => {
    render(
      <Table
        nodes={nodes}
        selectedIndex={-1}
        onSelect={() => {}}
        onActivate={() => {}}
        onContextMenu={() => {}}
      />
    );

    // Click Size header to switch to ascending
    const sizeHeader = screen.getByText(/Size/);
    fireEvent.click(sizeHeader);

    const rows = screen.getAllByTestId(/^table-row-/);
    expect(rows[0]).toHaveTextContent("small.rs");
    expect(rows[2]).toHaveTextContent("big.txt");
  });

  it("selects row on click", () => {
    const onSelect = vi.fn();
    render(
      <Table
        nodes={nodes}
        selectedIndex={-1}
        onSelect={onSelect}
        onActivate={() => {}}
        onContextMenu={() => {}}
      />
    );
    fireEvent.click(screen.getByTestId("table-row-big.txt"));
    expect(onSelect).toHaveBeenCalledWith(0);
  });

  it("activates row on double click", () => {
    const onActivate = vi.fn();
    render(
      <Table
        nodes={nodes}
        selectedIndex={-1}
        onSelect={() => {}}
        onActivate={onActivate}
        onContextMenu={() => {}}
      />
    );
    fireEvent.doubleClick(screen.getByTestId("table-row-big.txt"));
    expect(onActivate).toHaveBeenCalledWith(
      expect.objectContaining({ name: "big.txt" })
    );
  });

  it("shows context menu on right click", () => {
    const onContextMenu = vi.fn();
    render(
      <Table
        nodes={nodes}
        selectedIndex={-1}
        onSelect={() => {}}
        onActivate={() => {}}
        onContextMenu={onContextMenu}
      />
    );
    fireEvent.contextMenu(screen.getByTestId("table-row-image.png"), {
      clientX: 200,
      clientY: 300,
    });
    expect(onContextMenu).toHaveBeenCalledWith(
      expect.objectContaining({ name: "image.png" }),
      200,
      300
    );
  });

  it("shows empty state when no nodes", () => {
    render(
      <Table
        nodes={[]}
        selectedIndex={-1}
        onSelect={() => {}}
        onActivate={() => {}}
        onContextMenu={() => {}}
      />
    );
    expect(screen.getByTestId("table-empty")).toBeInTheDocument();
  });
});
