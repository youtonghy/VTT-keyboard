import { readFile, rm, stat } from "node:fs/promises";
import { arch } from "node:os";
import { basename, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
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
detachMountedImage(outputDmg);
if (await hasValidDmg(outputDmg)) {
  console.warn(`DMG already exists after Tauri retry: ${outputDmg}`);
  process.exit(0);
}
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
if (code !== 0) {
  process.exit(code ?? 1);
}

if (!(await hasValidDmg(outputDmg))) {
  console.error(`DMG retry finished without a valid output file: ${outputDmg}`);
  process.exit(1);
}

console.warn(`DMG retry succeeded: ${outputDmg}`);
process.exit(0);

function detachMountedImage(imagePath) {
  const info = spawnSync("hdiutil", ["info"], { encoding: "utf8" });
  if (info.status !== 0 || !info.stdout.includes(imagePath)) {
    return;
  }

  const entries = info.stdout.split("================================================");
  for (const entry of entries) {
    if (!entry.includes(`image-path      : ${imagePath}`)) {
      continue;
    }
    const devices = [...entry.matchAll(/^\/dev\/(disk\d+)\b/gm)].map((match) => match[1]);
    const rootDevice = devices.find((device) => !devices.some((other) => other !== device && other.startsWith(device)));
    if (rootDevice) {
      spawnSync("hdiutil", ["detach", `/dev/${rootDevice}`], { stdio: "inherit" });
    }
  }
}

async function hasValidDmg(imagePath) {
  const outputStats = await stat(imagePath).catch(() => null);
  return Boolean(outputStats?.isFile() && outputStats.size > 0);
}
