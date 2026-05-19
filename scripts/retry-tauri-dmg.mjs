import { readFile, rm, stat } from "node:fs/promises";
import { arch } from "node:os";
import { basename, resolve } from "node:path";
import { spawn } from "node:child_process";
import { cleanTauriBundleArtifacts, rootDir } from "./clean-tauri-bundle-artifacts.mjs";

const tauriConfig = JSON.parse(
  await readFile(resolve(rootDir, "src-tauri/tauri.conf.json"), "utf8"),
);
const cargoToml = await readFile(resolve(rootDir, "src-tauri/Cargo.toml"), "utf8");

const productName = tauriConfig.productName;
const version = tauriConfig.version ?? cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const targetArch = { arm64: "aarch64", x64: "x64" }[arch()] ?? arch();

if (!productName || !version) {
  throw new Error("Cannot determine Tauri product name or version for DMG retry.");
}

const bundleDir = resolve(rootDir, "src-tauri/target/release/bundle");
const dmgDir = resolve(bundleDir, "dmg");
const macosDir = resolve(bundleDir, "macos");
const appName = `${productName}.app`;
const outputDmg = resolve(dmgDir, `${productName}_${version}_${targetArch}.dmg`);
const script = resolve(dmgDir, "bundle_dmg.sh");
const icon = resolve(dmgDir, "icon.icns");
const appPath = resolve(macosDir, appName);

await cleanTauriBundleArtifacts();
await rm(outputDmg, { force: true });

const appSizeBytes = (await stat(appPath)).size;
const diskImageSizeMb = Math.max(512, Math.ceil(appSizeBytes / 1024 / 1024) + 160);

const args = [
  script,
  "--volname",
  productName,
  "--volicon",
  icon,
  "--icon",
  appName,
  "128",
  "128",
  "--app-drop-link",
  "372",
  "128",
  "--disk-image-size",
  String(diskImageSizeMb),
  "--skip-jenkins",
  outputDmg,
  macosDir,
];

console.warn(
  `Retrying DMG bundling with ${basename(script)} --disk-image-size ${diskImageSizeMb} --skip-jenkins`,
);

const child = spawn("bash", args, { cwd: rootDir, stdio: "inherit" });
const code = await new Promise((resolveCode) => child.on("close", resolveCode));
process.exit(code ?? 1);
