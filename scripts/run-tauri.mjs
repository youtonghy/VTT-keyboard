import { spawn } from "node:child_process";
import { resolve } from "node:path";
import {
  cleanTauriBundleArtifacts,
  rootDir,
  tauriBundleDirsForArgs,
} from "./clean-tauri-bundle-artifacts.mjs";

const args = process.argv.slice(2);
const tauriBin = resolve(rootDir, "node_modules/.bin/tauri");
const env = { ...process.env };

function run(command, commandArgs) {
  const child = spawn(command, commandArgs, { cwd: rootDir, stdio: "inherit", env });
  return new Promise((resolveCode) => child.on("close", resolveCode));
}

function isDmgBuild(commandArgs) {
  if (process.platform !== "darwin") {
    return false;
  }
  const bundleFlagIndex = commandArgs.findIndex((arg) => arg === "--bundles" || arg === "-b");
  const explicitDmgBundle =
    bundleFlagIndex >= 0 && commandArgs[bundleFlagIndex + 1]?.split(",").includes("dmg");

  return commandArgs[0] === "build" && (explicitDmgBundle || !commandArgs.includes("--bundles"));
}

if (args[0] === "build") {
  // linuxdeploy ships an old strip that cannot parse `.relr.dyn` sections used by
  // newer distro libraries (e.g. Arch), which makes AppImage bundling fail.
  if (process.platform === "linux" && !env.CI && env.NO_STRIP === undefined) {
    env.NO_STRIP = "true";
  }

  // Updater artifacts require the signing key; skip them for local unsigned builds.
  if (!env.TAURI_SIGNING_PRIVATE_KEY) {
    console.warn("TAURI_SIGNING_PRIVATE_KEY is not set; building without updater artifacts.");
    args.push("--config", JSON.stringify({ bundle: { createUpdaterArtifacts: false } }));
  }
}

await cleanTauriBundleArtifacts(tauriBundleDirsForArgs(args));

const code = await run(tauriBin, args);

if (code !== 0 && isDmgBuild(args)) {
  console.warn("Tauri DMG bundling failed; retrying with the project DMG fallback.");
  const retryCode = await run("bun", [resolve(rootDir, "scripts/retry-tauri-dmg.mjs")]);
  if (retryCode === 0) {
    console.warn("DMG fallback completed successfully.");
  }
  process.exit(retryCode ?? 1);
}

process.exit(code ?? 1);
