import { formatMessage, type Translations } from "../../i18n";
import type { KeptInfo, ResolutionUnit } from "../../ipc";

const CM_TO_INCH = 2.54;
const M_TO_INCH = 0.0254;

/**
 * Converts a resolution value in the specified unit to dpi, rounded to nearest integer (design §4.7).
 */
export function convertToDpi(val: number, unit: ResolutionUnit): number {
  if (unit === "centimeter") {
    return Math.round(val * CM_TO_INCH);
  }
  if (unit === "meter") {
    return Math.round(val * M_TO_INCH);
  }
  return Math.round(val);
}

export type OrientationCode = 2 | 3 | 4 | 5 | 6 | 7 | 8;

/**
 * Type guard for orientation values preserved when cleaning (design §4.7).
 */
export function isOrientationCode(value: number): value is OrientationCode {
  return (
    value === 2 ||
    value === 3 ||
    value === 4 ||
    value === 5 ||
    value === 6 ||
    value === 7 ||
    value === 8
  );
}

/**
 * Formats orientation KeptInfo into localized string (e.g. "向き（右に 90 度回転）").
 * Value 1 is not preserved so returns null.
 */
export function formatOrientation(
  value: number,
  t: Translations,
): string | null {
  if (!isOrientationCode(value)) {
    return null;
  }
  const desc = t.details.orientation[value];
  return formatMessage(t.details.orientationLabel, { desc });
}

/**
 * Formats color profile KeptInfo into localized string (e.g. "色のプロファイル（sRGB）" or "色のプロファイル").
 */
export function formatColorProfile(
  description: string | null,
  t: Translations,
): string {
  if (description !== null && description.trim() !== "") {
    return formatMessage(t.details.colorProfileLabel, { desc: description });
  }
  return t.details.colorProfilePlain;
}

/**
 * Formats resolution KeptInfo into localized string (e.g. "解像度（300 dpi）" or "解像度（300 × 600 dpi）").
 */
export function formatResolution(
  x: number,
  y: number,
  unit: ResolutionUnit,
  t: Translations,
): string {
  const xDpi = convertToDpi(x, unit);
  const yDpi = convertToDpi(y, unit);
  const dpiText = xDpi === yDpi ? `${xDpi}` : `${xDpi} × ${yDpi}`;
  return formatMessage(t.details.resolutionLabel, { dpi: dpiText });
}

/**
 * Formats the entire array of KeptInfo into a single comma-separated string (design §10.1, mockup BList.dc.html).
 */
export function formatKeptInfo(
  kept: readonly KeptInfo[],
  t: Translations,
): string {
  const parts: string[] = [];
  for (const item of kept) {
    if (item.type === "orientation") {
      const formatted = formatOrientation(item.value, t);
      if (formatted !== null) {
        parts.push(formatted);
      }
    } else if (item.type === "colorProfile") {
      parts.push(formatColorProfile(item.description, t));
    } else if (item.type === "resolution") {
      parts.push(formatResolution(item.x, item.y, item.unit, t));
    }
  }
  return parts.join(t.details.keptSeparator);
}
