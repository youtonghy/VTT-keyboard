import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { rootDir } from "./clean-tauri-bundle-artifacts.mjs";

const EXPECTED_PRODUCT_NAME = "vtt-keyboard";
const EXPECTED_IDENTIFIER = "com.youtonghy.vtt-keyboard";
const EXPECTED_MACOS_BUNDLE_NAME = "VTT Keyboard";

const configPath = resolve(rootDir, "src-tauri/tauri.conf.json");
const tauriConfig = JSON.parse(await readFile(configPath, "utf8"));
const macosConfig = tauriConfig.bundle?.macOS ?? {};
const failures = [];

assertEqual(
  "productName",
  tauriConfig.productName,
  EXPECTED_PRODUCT_NAME,
  "macOS uses this for the .app directory and TCC permission inheritance depends on keeping it stable.",
);
assertEqual(
  "identifier",
  tauriConfig.identifier,
  EXPECTED_IDENTIFIER,
  "macOS writes this to CFBundleIdentifier, so changing it makes the app look new to system permissions.",
);
assertEqual(
  "bundle.macOS.bundleName",
  macosConfig.bundleName,
  EXPECTED_MACOS_BUNDLE_NAME,
  "Use this for the readable display name instead of changing productName.",
);

if (failures.length > 0) {
  console.error("App identity validation failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("App identity validation passed.");

function assertEqual(field, actual, expected, reason) {
  if (actual === expected) {
    return;
  }

  failures.push(`${field} expected ${JSON.stringify(expected)} but got ${JSON.stringify(actual)}. ${reason}`);
}
