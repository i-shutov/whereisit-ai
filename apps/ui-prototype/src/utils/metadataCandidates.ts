export interface MetadataCandidateRecord {
  name: string;
  size: number;
  volumeId: number;
}

export function metadataCandidateKey(name: string, size: number): string {
  return `${name.normalize('NFC').trim().toLocaleLowerCase('en-US')}\u0000${size}`;
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
