export interface DemoAnnotation {
  description: string;
  tags: string;
  category: string;
}

export interface DemoAnnotationRecord {
  base: DemoAnnotation;
  value: DemoAnnotation;
  external: DemoAnnotation | null;
  saved?: boolean;
}

export const PRIMARY_DEMO_DOCUMENT_ID = 'synthetic-demo-primary-ctf';
export const COPY_DEMO_DOCUMENT_ID = 'synthetic-demo-byte-identical-copy';

export const demoDocumentKey = (documentId: string, volumeId: number, entryId: number) =>
  `whereisit-demo-annotations:v1:${encodeURIComponent(documentId)}:${volumeId}:${entryId}`;

export const initialDemoRecord = (value: DemoAnnotation): DemoAnnotationRecord => ({
  base: { ...value }, value: { ...value }, external: null, saved: true,
});

export const markDemoRecordSaved = (record: DemoAnnotationRecord): DemoAnnotationRecord => ({ ...record, saved: true });

export const demoRecordHasConflict = (record: DemoAnnotationRecord) =>
  conflictFields(record).length > 0;

export const conflictFields = (record: DemoAnnotationRecord): (keyof DemoAnnotation)[] => {
  if (!record.external) return [];
  return (['description', 'tags', 'category'] as const).filter((field) =>
    record.value[field] !== record.base[field] &&
    record.external![field] !== record.base[field] &&
    record.value[field] !== record.external![field]
  );
};

export const applyExternalDemoChange = (record: DemoAnnotationRecord, description: string): DemoAnnotationRecord => {
  const external = { ...record.base, description };
  const value = { ...record.value };
  for (const field of ['description', 'tags', 'category'] as const) {
    if (record.value[field] === record.base[field] && external[field] !== record.base[field]) value[field] = external[field];
  }
  const candidate = { ...record, value, external };
  if (conflictFields(candidate).length === 0) {
    return { ...candidate, base: { ...external }, external: null };
  }
  return candidate;
};

export const resolveDemoConflict = (record: DemoAnnotationRecord, choice: 'mine' | 'external'): DemoAnnotationRecord => {
  if (!record.external) return record;
  const conflicts = conflictFields(record);
  const value = { ...record.value };
  for (const field of ['description', 'tags', 'category'] as const) {
    const changedExternally = record.external[field] !== record.base[field];
    const changedLocally = record.value[field] !== record.base[field];
    if (changedExternally && (!changedLocally || (conflicts.includes(field) && choice === 'external'))) {
      value[field] = record.external[field];
    }
  }
  return { base: { ...record.external }, value, external: null, saved: true };
};

export const parseDemoRecord = (raw: string): DemoAnnotationRecord => {
  const candidate: unknown = JSON.parse(raw);
  const validAnnotation = (value: unknown): value is DemoAnnotation =>
    !!value && typeof value === 'object' && ['description', 'tags', 'category'].every((key) => typeof (value as Record<string, unknown>)[key] === 'string');
  if (!candidate || typeof candidate !== 'object') throw new Error('Invalid synthetic annotation record');
  const record = candidate as Record<string, unknown>;
  if (!validAnnotation(record.base) || !validAnnotation(record.value) || (record.external !== null && !validAnnotation(record.external)) || (record.saved !== undefined && typeof record.saved !== 'boolean')) {
    throw new Error('Invalid synthetic annotation record');
  }
  return { base: { ...record.base }, value: { ...record.value }, external: record.external ? { ...record.external } : null, saved: typeof record.saved === 'boolean' ? record.saved : false };
};

export const sameAnnotation = (a: DemoAnnotation, b: DemoAnnotation) =>
  a.description === b.description && a.tags === b.tags && a.category === b.category;
