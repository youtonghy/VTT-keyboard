/** 将逗号分隔的字符串解析为去空格的数组（同时支持中英文逗号） */
export const parseList = (value: string): string[] =>
  value
    .split(/[,，]/)
    .map((item) => item.trim())
    .filter(Boolean);

/** 将数组转为逗号分隔字符串 */
export const listToString = (values: string[]): string => values.join(", ");

/** 规范化阿里云地域（仅 singapore / beijing） */
export const normalizeAliyunRegion = (
  value: string | undefined,
): "singapore" | "beijing" =>
  value === "singapore" ? "singapore" : "beijing";

/** 将未知错误转为可读字符串 */
export const toErrorMessage = (error: unknown): string => {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
};
