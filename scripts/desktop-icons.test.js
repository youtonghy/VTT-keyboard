import { readFileSync } from "node:fs";
import { describe, expect, test } from "bun:test";

const tauriDir = new URL("../src-tauri/", import.meta.url);

function readAsset(path) {
  return readFileSync(new URL(path, tauriDir));
}

function checkRgbaPng(path, size) {
  const png = readAsset(path);
  expect(png.subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  expect(png.readUInt32BE(16)).toBe(size);
  expect(png.readUInt32BE(20)).toBe(size);
  expect(png[24]).toBe(8);
  expect(png[25]).toBe(6);
}

for (const [variant, configFile, iconDir] of [
  ["light", "tauri.conf.json", "icons"],
  ["dark", "tauri.dark-icon.conf.json", "icons/dark"],
]) {
  describe(`${variant} desktop icons`, () => {
    const { bundle } = JSON.parse(readAsset(configFile).toString());
    const iconPaths = Object.fromEntries(bundle.icon.map((path) => [path.split("/").at(-1), path]));

    test("bundles square 8-bit RGBA PNGs at the expected sizes", () => {
      for (const [name, size] of [["32x32.png", 32], ["128x128.png", 128], ["128x128@2x.png", 256]]) {
        expect(iconPaths[name]).toBe(`${iconDir}/${name}`);
        checkRgbaPng(iconPaths[name], size);
      }
      checkRgbaPng(`icons/source/${variant}.png`, 1024);
    });

    test("bundles a macOS ICNS and a Windows ICO with the required sizes", () => {
      expect(iconPaths["icon.icns"]).toBe(`${iconDir}/icon.icns`);
      const icns = readAsset(iconPaths["icon.icns"]);
      expect(icns.toString("ascii", 0, 4)).toBe("icns");
      expect(icns.readUInt32BE(4)).toBe(icns.length);

      expect(iconPaths["icon.ico"]).toBe(`${iconDir}/icon.ico`);
      const ico = readAsset(iconPaths["icon.ico"]);
      expect(ico.readUInt32LE(0)).toBe(0x00010000);
      const sizes = new Set();
      for (let i = 0; i < ico.readUInt16LE(4); i++) {
        const offset = 6 + i * 16;
        expect(ico[offset]).toBe(ico[offset + 1]);
        sizes.add(ico[offset] || 256);
      }
      expect(sizes).toEqual(new Set([16, 24, 32, 48, 64, 256]));
    });
  });
}
