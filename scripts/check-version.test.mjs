import { describe, expect, it } from "vitest";
import { cargoVersion, findVersionMismatch } from "./check-version.mjs";

describe("cargoVersion", () => {
  it("reads the version of the [package] section", () => {
    const toml = `[package]\nname = "x"\nversion = "1.2.3"\n\n[dependencies]\nserde = { version = "1" }\n`;

    expect(cargoVersion(toml)).toBe("1.2.3");
  });

  it("does not take the version of a dependency", () => {
    const toml = `[package]\nname = "x"\n\n[dependencies]\nversion = "9.9.9"\n`;

    expect(cargoVersion(toml)).toBeNull();
  });

  it("returns null without a package section", () => {
    expect(cargoVersion(`[workspace]\nmembers = []\n`)).toBeNull();
  });
});

describe("findVersionMismatch", () => {
  it("accepts equal versions", () => {
    expect(findVersionMismatch({ a: "0.1.0", b: "0.1.0", c: "0.1.0" })).toBeNull();
  });

  it("lists each file and version when they diverge", () => {
    const message = findVersionMismatch({ a: "0.1.0", b: "0.2.0", c: "0.1.0" });

    expect(message).toContain("divergem");
    expect(message).toContain("a: 0.1.0");
    expect(message).toContain("b: 0.2.0");
  });

  it("reports the files whose version is missing", () => {
    const message = findVersionMismatch({ a: "0.1.0", b: null, c: undefined });

    expect(message).toContain("b");
    expect(message).toContain("c");
    expect(message).not.toContain("a,");
  });
});
