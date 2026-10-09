import { describe, expect, it } from "vitest";
import { parseThirdPartyLicenses, thirdPartyLicenses } from ".";

describe("third-party licenses", () => {
  it("includes runtime dependencies and excludes dev-only crates", () => {
    const cargoPackages = new Set(
      thirdPartyLicenses.flatMap((license) =>
        license.packages
          .filter((pkg) => pkg.ecosystem === "cargo")
          .map((pkg) => pkg.name),
      ),
    );
    expect(cargoPackages).toContain("lopdf");
    expect(cargoPackages).toContain("kamadak-exif");
    expect(cargoPackages).not.toContain("hayro");
    expect(cargoPackages).not.toContain("jpeg-encoder");
    expect(cargoPackages).not.toContain("image");
  });

  it("include Rust crates and npm packages", () => {
    const ecosystems = new Set(
      thirdPartyLicenses.flatMap((license) =>
        license.packages.map((pkg) => pkg.ecosystem),
      ),
    );
    expect(ecosystems).toEqual(new Set(["cargo", "npm"]));
  });

  it("rejects a list of the wrong shape", () => {
    expect(() => parseThirdPartyLicenses({})).toThrow();
    expect(() =>
      parseThirdPartyLicenses({
        licenses: [
          {
            id: "MIT",
            name: "MIT License",
            text: "",
            packages: [{ name: "a", version: "1", ecosystem: "pip" }],
          },
        ],
      }),
    ).toThrow();
  });
});
