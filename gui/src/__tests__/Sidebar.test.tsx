import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Sidebar } from "../components/Sidebar";

describe("Sidebar", () => {
  it("renders directory input and scan button", () => {
    render(<Sidebar onScan={() => {}} result={null} totalSize={0} />);
    expect(screen.getByLabelText("Directory")).toBeInTheDocument();
    expect(screen.getByText("Scan")).toBeInTheDocument();
  });

  it("calls onScan with entered path", () => {
    const onScan = vi.fn();
    render(<Sidebar onScan={onScan} result={null} totalSize={0} />);
    fireEvent.change(screen.getByLabelText("Directory"), {
      target: { value: "/home/user" },
    });
    fireEvent.click(screen.getByText("Scan"));
    expect(onScan).toHaveBeenCalledWith("/home/user");
  });

  it("displays stats when result provided", () => {
    render(
      <Sidebar
        onScan={() => {}}
        result={{
          path: "/home/user",
          name: "user",
          size: 1024,
          modified: "",
          node_type: "Other",
          is_dir: true,
          children: [],
        }}
        totalSize={1024}
      />
    );
    expect(screen.getByTestId("sidebar-stats")).toBeInTheDocument();
    expect(screen.getByText("1.0 KB")).toBeInTheDocument();
  });
});
