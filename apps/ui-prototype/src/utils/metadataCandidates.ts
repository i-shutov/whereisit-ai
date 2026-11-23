export interface MetadataCandidateRecord {
  name: string;
  size: number;
  volumeId: number;
}

export interface MetadataCandidateHit extends MetadataCandidateRecord {
  volumeLabel: string;
  relativePath: string;
}

export function normalizeMetadataName(name: string): string {
  return name.normalize('NFC').trim().toLocaleLowerCase('en-US');
}

export function metadataCandidateKey(name: string, size: number): string {
  return `${normalizeMetadataName(name)}\u0000${size}`;
}

export function countCandidateVolumes(records: MetadataCandidateRecord[]): Map<string, number> {
  const volumes = new Map<string, Set<number>>();
  for (const record of records) {
    const key = metadataCandidateKey(record.name, record.size);
    const set = volumes.get(key) ?? new Set<number>();
    set.add(record.volumeId);
    volumes.set(key, set);
  }
  return new Map(Array.from(volumes, ([key, set]) => [key, set.size]));
}

export function distinctCandidateVolumeRecords<T extends MetadataCandidateRecord>(records: T[]): T[] {
  const seen = new Set<string>();
  const result: T[] = [];
  for (const record of records) {
    const key = `${metadataCandidateKey(record.name, record.size)}\u0000${record.volumeId}`;
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(record);
  }
  return result;
}

export function buildMetadataCandidateIndex<T extends MetadataCandidateHit>(records: T[]): Map<string, T[]> {
  const grouped = new Map<string, T[]>();
  for (const record of records) {
    const key = metadataCandidateKey(record.name, record.size);
    const hits = grouped.get(key) ?? [];
    grouped.set(key, hits);
    if (!hits.some((hit) => hit.volumeId === record.volumeId)) hits.push(record);
  }
  return grouped;
}
