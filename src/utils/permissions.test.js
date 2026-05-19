import { describe, expect, test } from "bun:test";
import { canStartMicrophoneTest } from "./permissions.ts";

describe("canStartMicrophoneTest", () => {
  test("allows microphone tests on non-macOS platforms", () => {
    expect(canStartMicrophoneTest(false, "denied")).toBe(true);
  });

  test("allows microphone tests when macOS microphone permission is approved", () => {
    expect(canStartMicrophoneTest(true, "authorized")).toBe(true);
    expect(canStartMicrophoneTest(true, "approved")).toBe(true);
  });

  test("blocks microphone tests when macOS microphone permission is missing", () => {
    expect(canStartMicrophoneTest(true, "notDetermined")).toBe(false);
    expect(canStartMicrophoneTest(true, "restricted")).toBe(false);
    expect(canStartMicrophoneTest(true, "denied")).toBe(false);
    expect(canStartMicrophoneTest(true, "unknown")).toBe(false);
  });
});
