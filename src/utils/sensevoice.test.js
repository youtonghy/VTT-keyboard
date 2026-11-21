import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  LOCAL_MODELS,
  LOCAL_MODEL_FAMILIES,
  isStreamingLocalModel,
} from "./sensevoice.ts";

// 前端模型表是 src-tauri/src/sensevoice/model.rs 的手写副本，这里校验两边一致。
const rustSource = readFileSync(
  join(import.meta.dir, "../../src-tauri/src/sensevoice/model.rs"),
  "utf8",
);

const rustConstants = new Map(
  [...rustSource.matchAll(/pub const (LOCAL_MODEL_[A-Z0-9_]+): &str = "([^"]+)";/g)].map(
    (m) => [m[1], m[2]],
  ),
);

const rustSpecs = [...rustSource.matchAll(/const SPEC_[A-Z0-9_]+: LocalModelSpec = LocalModelSpec \{([\s\S]*?)\n\};/g)].map(
  (m) => {
    const body = m[1];
    const field = (name) => body.match(new RegExp(`${name}: ([^,]+),`))[1].trim();
    const resolve = (value) =>
      value.startsWith('"') ? JSON.parse(value) : rustConstants.get(value);
    return {
      key: resolve(field("model_key")),
      family: resolve(field("family")),
      displayName: JSON.parse(field("display_name")),
      modelId: JSON.parse(field("model_id")),
      kind: field("kind"),
    };
  },
);

describe("local model catalog", () => {
  test("matches the Rust model table", () => {
    const byKey = (a, b) => a.key.localeCompare(b.key);
    const frontend = LOCAL_MODELS.map(({ key, family, displayName, modelId }) => ({
      key,
      family,
      displayName,
      modelId,
    })).sort(byKey);
    const backend = rustSpecs
      .map(({ key, family, displayName, modelId }) => ({ key, family, displayName, modelId }))
      .sort(byKey);
    expect(backend.length).toBeGreaterThan(0);
    expect(frontend).toEqual(backend);
  });

  test("every model family has a label", () => {
    const families = new Set(LOCAL_MODELS.map((spec) => spec.family));
    expect(new Set(LOCAL_MODEL_FAMILIES.map((family) => family.key))).toEqual(families);
  });

  test("streaming flag matches the Rust model kind", () => {
    for (const spec of rustSpecs) {
      expect(isStreamingLocalModel(spec.key)).toBe(
        spec.kind === "LocalModelKind::OnlineTransducer",
      );
    }
  });
});
