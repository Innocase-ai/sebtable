export type RecordValue = unknown;

export interface Record {
  _id: string;
  [fieldId: string]: unknown;
}

export interface PaginatedRecords {
  records: Record[];
  total: number;
  page: number;
  page_size: number;
}
