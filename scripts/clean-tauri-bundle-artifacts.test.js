import { mkdir, mkdtemp, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, test } from "bun:test";
import {
  cleanTauriBundleArtifacts,
  getCliOptionValue,
  rootDir,
  tauriBundleDirsForArgs,
} from "./clean-tauri-bundle-artifacts.mjs";

describe("cleanTauriBundleArtifacts", () => {
  test("skips bundle directories that do not exist", async () => {
    const tempDir = await mkdtemp(join(tmpdir(), "vtt-tauri-bundle-clean-"));
    await expect(cleanTauriBundleArtifacts([join(tempDir, "missing")])).resolves.toBeUndefined();
  });

  test("removes only temporary DMG artifacts from existing bundle directories", async () => {
    const bundleDir = await mkdtemp(join(tmpdir(), "vtt-tauri-bundle-clean-"));
    const dmgDir = join(bundleDir, "dmg");
    const temporaryDmg = join(dmgDir, "rw.temp.dmg");
    const finalDmg = join(dmgDir, "VTT Keyboard_0.1.0_aarch64.dmg");

    await mkdir(dmgDir);
    await writeFile(temporaryDmg, "temporary");
    await writeFile(finalDmg, "final");

    await cleanTauriBundleArtifacts([bundleDir]);

    await expect(stat(temporaryDmg)).rejects.toMatchObject({ code: "ENOENT" });
    await expect(stat(finalDmg)).resolves.toMatchObject({ size: 5 });
  });
});

describe("tauriBundleDirsForArgs", () => {
  test("includes target-specific bundle output when --target is provided", () => {
    const dirs = tauriBundleDirsForArgs([
      "build",
      "--target",
      "x86_64-pc-windows-msvc",
      "--config",
      ".ci-tauri-version.json",
    ]);

    expect(dirs).toContain(
      join(rootDir, "src-tauri/target/x86_64-pc-windows-msvc/release/bundle"),
    );
    expect(dirs).toContain(join(rootDir, "src-tauri/target/release/bundle"));
  });

  test("supports equals-style target flags", () => {
    expect(getCliOptionValue(["build", "--target=x86_64-pc-windows-msvc"], "--target", "-t")).toBe(
      "x86_64-pc-windows-msvc",
    );
    expect(getCliOptionValue(["build", "-t=aarch64-apple-darwin"], "--target", "-t")).toBe(
      "aarch64-apple-darwin",
    );
  });
});
