import { rm } from "node:fs/promises";
import { glob } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const currentFile = fileURLToPath(import.meta.url);
export const rootDir = resolve(fileURLToPath(import.meta.url), "../..");
export const bundleDir = resolve(rootDir, "src-tauri/target/release/bundle");

export async function cleanTauriBundleArtifacts() {
  for await (const artifact of glob("**/rw.*.dmg", { cwd: bundleDir })) {
    await rm(resolve(bundleDir, artifact), { force: true });
  }
}

if (process.argv[1] === currentFile) {
  await cleanTauriBundleArtifacts();
}
