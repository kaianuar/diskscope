import type { FileType } from "./domain";

/** Color palette keyed by FileType. */
const TYPE_COLORS: Record<FileType, string> = {
  Image: "#4A90D9",
  Video: "#E74C3C",
  Audio: "#9B59B6",
  Document: "#F39C12",
  Code: "#2ECC71",
  Archive: "#1ABC9C",
  Other: "#95A5A6",
};

/** Lighter variant for treemap fill. */
const TYPE_COLORS_LIGHT: Record<FileType, string> = {
  Image: "#7EB3E8",
  Video: "#F1948A",
  Audio: "#C39BD3",
  Document: "#F8C471",
  Code: "#82E0AA",
  Archive: "#76D7C4",
  Other: "#BDC3C7",
};

export function getTypeColor(type: FileType): string {
  return TYPE_COLORS[type] ?? TYPE_COLORS.Other;
}

export function getTypeColorLight(type: FileType): string {
  return TYPE_COLORS_LIGHT[type] ?? TYPE_COLORS_LIGHT.Other;
}

/** Format bytes as human-readable string. */
export function humanSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const val = bytes / Math.pow(1024, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

/** Format ISO date string to locale short date. */
export function humanDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString();
  } catch {
    return iso;
  }
}
