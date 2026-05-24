import React, { useState, useRef, useLayoutEffect } from "react";
import { createPortal } from "react-dom";
import { calculateFloatingPosition, type FloatingPlacement } from "../utils/floating";

interface TooltipProps {
  content: React.ReactNode;
  children: React.ReactElement;
  position?: FloatingPlacement;
}

export function Tooltip({ content, children, position = "top" }: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const [floatingPosition, setFloatingPosition] = useState<{
    top: number;
    left: number;
    placement: FloatingPlacement;
  } | null>(null);
  const wrapperRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showTooltip = () => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    setFloatingPosition(null);
    setVisible(true);
  };

  const hideTooltip = () => {
    timeoutRef.current = setTimeout(() => {
      setVisible(false);
      setFloatingPosition(null);
    }, 100);
  };

  useLayoutEffect(() => {
    if (!visible || !wrapperRef.current || !contentRef.current) {
      return;
    }

    const updatePosition = () => {
      if (!wrapperRef.current || !contentRef.current) {
        return;
      }
      const triggerRect = wrapperRef.current.getBoundingClientRect();
      const contentRect = contentRef.current.getBoundingClientRect();
      setFloatingPosition(
        calculateFloatingPosition({
          triggerRect,
          floatingSize: {
            width: contentRect.width,
            height: contentRect.height,
          },
          viewport: {
            width: window.innerWidth,
            height: window.innerHeight,
          },
          preferredPlacement: position,
        })
      );
    };

    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [position, visible, content]);

  if (!content) return children;

  return (
    <div
      ref={wrapperRef}
      className="tooltip-wrapper"
      onMouseEnter={showTooltip}
      onMouseLeave={hideTooltip}
      onFocus={showTooltip}
      onBlur={hideTooltip}
    >
      {children}
      {visible && createPortal(
        <div
          ref={contentRef}
          className={`tooltip-content tooltip-${floatingPosition?.placement ?? position}`}
          style={
            floatingPosition
              ? {
                  top: `${floatingPosition.top}px`,
                  left: `${floatingPosition.left}px`,
                  visibility: "visible",
                }
              : { top: 0, left: 0, visibility: "hidden" }
          }
        >
          {content}
        </div>,
        document.body
      )}
    </div>
  );
}
