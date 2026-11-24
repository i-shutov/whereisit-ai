export interface SearchJumpInput {
  volumeId: number | null;
  folderPrefix: string | null;
  folderEntryId?: number | null;
  query?: string;
  copySourceEntryId?: number | null;
}

export interface SearchJumpDefaults {
  mode: 'search' | 'browse' | 'copy_candidates';
  duplicateMode: 'video_rips';
  query: string;
  volumeId: number | null;
  parentId: number | null;
  folderPrefix: string | null;
  category: null;
  sourceEntryId: number | null;
  candidateVolumeId: null;
  offset: 0;
}

/** Fresh Search workspace context for an explicit jump from another workspace. */
export function buildSearchJumpDefaults(jump: SearchJumpInput): SearchJumpDefaults {
  const sourceEntryId = jump.copySourceEntryId ?? null;
  if (sourceEntryId !== null) {
    return {
      mode: 'copy_candidates',
      duplicateMode: 'video_rips',
      query: '',
      volumeId: null,
      parentId: null,
      folderPrefix: null,
      category: null,
      sourceEntryId,
      candidateVolumeId: null,
      offset: 0,
    };
  }

  const query = jump.query ?? '';
  const folderEntryId = Number.isInteger(jump.folderEntryId) && (jump.folderEntryId ?? 0) > 0 ? jump.folderEntryId! : null;
  const parentId = jump.volumeId === null ? null : folderEntryId;
  const knownBrowseTarget = parentId !== null || (jump.volumeId !== null && jump.folderPrefix === null);
  const mode = !query.trim() && knownBrowseTarget ? 'browse' : 'search';
  return {
    mode,
    duplicateMode: 'video_rips',
    query,
    volumeId: jump.volumeId,
    parentId,
    folderPrefix: jump.folderPrefix,
    category: null,
    sourceEntryId: null,
    candidateVolumeId: null,
    offset: 0,
  };
}
