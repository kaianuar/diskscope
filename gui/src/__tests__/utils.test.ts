import { describe, it, expect } from "vitest";
import { humanSize } from "../utils";

describe("humanSize", () => {
  it("formats zero bytes", () => {
    expect(humanSize(0)).toBe("0 B");
  });

  it("formats bytes", () => {
    expect(humanSize(500)).toBe("500 B");
  });

  it("formats kilobytes", () => {
    expect(humanSize(1024)).toBe("1.0 KB");
  });

  it("formats megabytes", () => {
    expect(humanSize(1048576)).toBe("1.0 MB");
  });

  it("formats gigabytes", () => {
    expect(humanSize(1073741824)).toBe("1.0 GB");
  });
});
