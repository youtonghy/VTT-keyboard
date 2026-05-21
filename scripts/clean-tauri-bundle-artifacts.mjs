import { rm, stat } from "node:fs/promises";
import { glob } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const currentFile = fileURLToPath(import.meta.url);
export const rootDir = resolve(fileURLToPath(import.meta.url), "../..");
export const bundleDir = resolve(rootDir, "src-tauri/target/release/bundle");

export function getCliOptionValue(args, longOption, shortOption) {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === longOption || arg === shortOption) {
      return args[index + 1];
    }

    if (arg.startsWith(`${longOption}=`) || (shortOption && arg.startsWith(`${shortOption}=`))) {
      return arg.slice(arg.indexOf("=") + 1);
    }
  }

  return undefined;
}

export function tauriBundleDirsForArgs(args) {
  const target = getCliOptionValue(args, "--target", "-t");
  const dirs = target
    ? [resolve(rootDir, "src-tauri/target", target, "release/bundle"), bundleDir]
    : [bundleDir];

  return [...new Set(dirs)];
}

export async function cleanTauriBundleArtifacts(bundleDirs = [bundleDir]) {
  for (const dir of bundleDirs) {
    const dirStats = await stat(dir).catch((error) => {
      if (error?.code === "ENOENT") {
        return null;
      }
      throw error;
    });

    if (!dirStats?.isDirectory()) {
      continue;
    }

    for await (const artifact of glob("**/rw.*.dmg", { cwd: dir })) {
      await rm(resolve(dir, artifact), { force: true });
    }
  }
}

if (process.argv[1] === currentFile) {
  await cleanTauriBundleArtifacts(tauriBundleDirsForArgs(process.argv.slice(2)));
}
