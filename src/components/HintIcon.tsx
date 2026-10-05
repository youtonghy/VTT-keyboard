import { Info } from "lucide-react";
import { Tooltip } from "./Tooltip";

/** 设置项旁的说明图标，悬停显示提示文本。 */
export function HintIcon({ content, size = 14 }: { content: string; size?: number }) {
  return (
    <Tooltip content={content}>
      <span className="inline-flex cursor-help text-muted-foreground transition-colors hover:text-foreground">
        <Info size={size} />
      </span>
    </Tooltip>
  );
}
