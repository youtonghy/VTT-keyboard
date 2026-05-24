export type FloatingPlacement = "top" | "bottom" | "left" | "right";

export interface FloatingRect {
  top: number;
  right: number;
  bottom: number;
  left: number;
  width: number;
  height: number;
}

interface FloatingSize {
  width: number;
  height: number;
}

interface FloatingViewport {
  width: number;
  height: number;
}

interface CalculateFloatingPositionOptions {
  triggerRect: FloatingRect;
  floatingSize: FloatingSize;
  viewport: FloatingViewport;
  preferredPlacement: FloatingPlacement;
  offset?: number;
  margin?: number;
}

interface FloatingPosition {
  top: number;
  left: number;
  placement: FloatingPlacement;
}

const fallbackPlacements: Record<FloatingPlacement, FloatingPlacement[]> = {
  top: ["top", "bottom", "right", "left"],
  bottom: ["bottom", "top", "right", "left"],
  left: ["left", "right", "top", "bottom"],
  right: ["right", "left", "top", "bottom"],
};

const clamp = (value: number, min: number, max: number) => {
  if (max < min) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
};

const getRawPosition = (
  triggerRect: FloatingRect,
  floatingSize: FloatingSize,
  placement: FloatingPlacement,
  offset: number,
) => {
  switch (placement) {
    case "top":
      return {
        top: triggerRect.top - floatingSize.height - offset,
        left: triggerRect.left + triggerRect.width / 2 - floatingSize.width / 2,
      };
    case "bottom":
      return {
        top: triggerRect.bottom + offset,
        left: triggerRect.left + triggerRect.width / 2 - floatingSize.width / 2,
      };
    case "left":
      return {
        top: triggerRect.top + triggerRect.height / 2 - floatingSize.height / 2,
        left: triggerRect.left - floatingSize.width - offset,
      };
    case "right":
      return {
        top: triggerRect.top + triggerRect.height / 2 - floatingSize.height / 2,
        left: triggerRect.right + offset,
      };
  }
};

const fitsMainAxis = (
  position: ReturnType<typeof getRawPosition>,
  floatingSize: FloatingSize,
  viewport: FloatingViewport,
  placement: FloatingPlacement,
  margin: number,
) => {
  switch (placement) {
    case "top":
      return position.top >= margin;
    case "bottom":
      return position.top + floatingSize.height <= viewport.height - margin;
    case "left":
      return position.left >= margin;
    case "right":
      return position.left + floatingSize.width <= viewport.width - margin;
  }
};

export function calculateFloatingPosition({
  triggerRect,
  floatingSize,
  viewport,
  preferredPlacement,
  offset = 8,
  margin = 12,
}: CalculateFloatingPositionOptions): FloatingPosition {
  const placement =
    fallbackPlacements[preferredPlacement].find((candidate) =>
      fitsMainAxis(
        getRawPosition(triggerRect, floatingSize, candidate, offset),
        floatingSize,
        viewport,
        candidate,
        margin,
      ),
    ) ?? preferredPlacement;
  const rawPosition = getRawPosition(triggerRect, floatingSize, placement, offset);

  return {
    top: clamp(rawPosition.top, margin, viewport.height - floatingSize.height - margin),
    left: clamp(rawPosition.left, margin, viewport.width - floatingSize.width - margin),
    placement,
  };
}
