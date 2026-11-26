export interface SearchOrderEntry {
  kind: 'file' | 'directory';
  size: number;
  nameNorm: string;
}

/** Preserve source traversal order for catalog requests; retain legacy size ordering otherwise. */
export function orderSearchMatches<T extends SearchOrderEntry>(
  matches: T[],
  order?: 'catalog' | 'size'
): T[] {
  if (order === 'catalog') return matches;
  return matches.sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === 'file' ? -1 : 1;
    return b.size - a.size || a.nameNorm.localeCompare(b.nameNorm);
  });
}
