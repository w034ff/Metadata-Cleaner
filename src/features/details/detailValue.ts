import { formatMessage, type Translations } from "../../i18n";
import type { DetailValue } from "../../ipc";

/**
 * Formats a DetailValue into a localized string (design §4.5, §4.7).
 */
export function formatDetailValue(value: DetailValue, t: Translations): string {
  switch (value.type) {
    case "text":
      return value.value;
    case "bytes":
      return formatMessage(t.detailValue.bytes, { n: value.value });
    case "count":
      return formatMessage(t.detailValue.count, { n: value.value });
  }
}
