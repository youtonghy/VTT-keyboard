import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { cleanTauriBundleArtifacts, rootDir } from "./clean-tauri-bundle-artifacts.mjs";

const args = process.argv.slice(2);
const tauriBin = resolve(rootDir, "node_modules/.bin/tauri");

function run(command, commandArgs) {
  const child = spawn(command, commandArgs, { cwd: rootDir, stdio: "inherit" });
  return new Promise((resolveCode) => child.on("close", resolveCode));
}

function isDmgBuild(commandArgs) {
  const bundleFlagIndex = commandArgs.findIndex((arg) => arg === "--bundles" || arg === "-b");
  const explicitDmgBundle =
    bundleFlagIndex >= 0 && commandArgs[bundleFlagIndex + 1]?.split(",").includes("dmg");

  return commandArgs[0] === "build" && (explicitDmgBundle || !commandArgs.includes("--bundles"));
}

await cleanTauriBundleArtifacts();

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
