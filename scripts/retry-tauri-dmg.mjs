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

const dmgConfig = tauriConfig.bundle?.macOS?.dmg ?? {};
const baseArgs = [
  script,
  "--volname",
  productName,
  "--volicon",
  icon,
  "--disk-image-size",
  String(diskImageSizeMb),
];
const iconArgs = dmgIconArgs(dmgConfig);

// First try with the configured window layout (needs Finder AppleScript access);
// fall back to an unstyled DMG so the build still produces an installer.
let code = await runBundleDmg(
  [...baseArgs, ...iconArgs, ...dmgWindowArgs(dmgConfig)],
  "with DMG window layout",
);
if (code !== 0) {
  await rm(outputDmg, { force: true });
  detachMountedImage(outputDmg);
  code = await runBundleDmg(
    [...baseArgs, ...iconArgs, "--skip-jenkins"],
    "without DMG window layout (--skip-jenkins)",
  );
}
if (code !== 0) {
  process.exit(code ?? 1);
}

if (!(await hasValidDmg(outputDmg))) {
  console.error(`DMG retry finished without a valid output file: ${outputDmg}`);
  process.exit(1);
}

console.warn(`DMG retry succeeded: ${outputDmg}`);
process.exit(0);

async function runBundleDmg(scriptArgs, label) {
  console.warn(
    `Retrying DMG bundling with ${basename(script)} --disk-image-size ${diskImageSizeMb} ${label}`,
  );
  const child = spawn("bash", [...scriptArgs, outputDmg, macosDir], { cwd: rootDir, stdio: "inherit" });
  return new Promise((resolveCode) => child.on("close", resolveCode));
}

function dmgIconArgs(dmg) {
  const app = dmg.appPosition ?? { x: 180, y: 170 };
  const folder = dmg.applicationFolderPosition ?? { x: 480, y: 170 };
  return ["--icon", appName, String(app.x), String(app.y), "--app-drop-link", String(folder.x), String(folder.y)];
}

function dmgWindowArgs(dmg) {
  const size = dmg.windowSize ?? { width: 660, height: 400 };
  const args = [
    "--window-size",
    String(size.width),
    String(size.height),
  ];
  if (dmg.windowPosition) {
    args.push("--window-pos", String(dmg.windowPosition.x), String(dmg.windowPosition.y));
  }
  if (dmg.background) {
    args.push("--background", resolve(rootDir, "src-tauri", dmg.background));
  }
  return args;
}

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
