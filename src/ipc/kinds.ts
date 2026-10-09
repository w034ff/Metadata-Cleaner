import type { Field } from "./generated/Field";
import type { MetadataKind } from "./generated/MetadataKind";

/**
 * Runtime list of all MetadataKind values in the order defined by design §4.5.
 */
export const METADATA_KINDS = [
  "location",
  "dateTime",
  "device",
  "author",
  "software",
  "comment",
  "thumbnail",
  "history",
  "other",
] as const satisfies readonly MetadataKind[];

// Type assertion at compile time ensuring METADATA_KINDS contains all MetadataKind values and no extras.
type MissingKinds = Exclude<MetadataKind, (typeof METADATA_KINDS)[number]>;
type ExtraKinds = Exclude<(typeof METADATA_KINDS)[number], MetadataKind>;
type ValidateMetadataKinds = [MissingKinds, ExtraKinds] extends [never, never]
  ? true
  : never;
const _assertAllKinds: ValidateMetadataKinds = true;
void _assertAllKinds;

/**
 * Runtime list of all Field values defined by design §4.5 and §4.7.
 */
export const FIELDS = [
  "latitude",
  "longitude",
  "city",
  "state",
  "country",
  "taken",
  "created",
  "modified",
  "cameraMake",
  "cameraModel",
  "serialNumber",
  "makerNote",
  "author",
  "copyright",
  "software",
  "pdfProducer",
  "title",
  "description",
  "comment",
  "subject",
  "keywords",
  "thumbnail",
  "earlierVersions",
  "xmp",
  "other",
] as const satisfies readonly Field[];

// Type assertion at compile time ensuring FIELDS contains all Field values and no extras.
type MissingFields = Exclude<Field, (typeof FIELDS)[number]>;
type ExtraFields = Exclude<(typeof FIELDS)[number], Field>;
type ValidateFields = [MissingFields, ExtraFields] extends [never, never]
  ? true
  : never;
const _assertAllFields: ValidateFields = true;
void _assertAllFields;
