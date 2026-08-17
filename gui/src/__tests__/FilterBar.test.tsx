import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { FilterBar } from "../components/FilterBar";
import type { ScanFilter } from "../domain";

describe("FilterBar", () => {
  it("renders filter inputs", () => {
    render(<FilterBar filter={{}} onChange={() => {}} />);
    expect(screen.getByPlaceholderText("1MB")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("1GB")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("*.log")).toBeInTheDocument();
  });

  it("renders file type checkboxes", () => {
    render(<FilterBar filter={{}} onChange={() => {}} />);
    expect(screen.getByLabelText("Image")).toBeInTheDocument();
    expect(screen.getByLabelText("Video")).toBeInTheDocument();
    expect(screen.getByLabelText("Code")).toBeInTheDocument();
  });

  it("calls onChange when toggling a file type", () => {
    const onChange = vi.fn();
    render(<FilterBar filter={{}} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText("Image"));
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ file_types: ["Image"] })
    );
  });

  it("updates filter view when filter changes", () => {
    const filter: ScanFilter = { file_types: ["Code"] };
    render(<FilterBar filter={filter} onChange={() => {}} />);
    expect(screen.getByLabelText("Code")).toBeChecked();
    expect(screen.getByLabelText("Image")).not.toBeChecked();
  });
});
