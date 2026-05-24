import { describe, expect, test } from "bun:test";
import { calculateFloatingPosition } from "./floating.ts";

const triggerRect = (rect) => ({
  width: rect.right - rect.left,
  height: rect.bottom - rect.top,
  ...rect,
});

describe("calculateFloatingPosition", () => {
  test("uses the preferred placement when there is enough room", () => {
    expect(
      calculateFloatingPosition({
        triggerRect: triggerRect({ top: 80, right: 140, bottom: 104, left: 100 }),
        floatingSize: { width: 120, height: 40 },
        viewport: { width: 320, height: 240 },
        preferredPlacement: "top",
      })
    ).toEqual({
      top: 32,
      left: 60,
      placement: "top",
    });
  });

  test("flips to the opposite side when the preferred side is clipped", () => {
    expect(
      calculateFloatingPosition({
        triggerRect: triggerRect({ top: 8, right: 140, bottom: 32, left: 100 }),
        floatingSize: { width: 120, height: 40 },
        viewport: { width: 320, height: 240 },
        preferredPlacement: "top",
      })
    ).toEqual({
      top: 40,
      left: 60,
      placement: "bottom",
    });
  });

  test("keeps the floating element inside the viewport margin", () => {
    expect(
      calculateFloatingPosition({
        triggerRect: triggerRect({ top: 120, right: 316, bottom: 144, left: 276 }),
        floatingSize: { width: 120, height: 40 },
        viewport: { width: 320, height: 240 },
        preferredPlacement: "bottom",
      })
    ).toEqual({
      top: 152,
      left: 188,
      placement: "bottom",
    });
  });
});
