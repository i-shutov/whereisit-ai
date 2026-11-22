export interface SearchJumpInput {
  volumeId: number | null;
  folderPrefix: string | null;
  query?: string;
  copySourceEntryId?: number | null;
}

export interface SearchJumpDefaults {
  mode: 'search' | 'copy_candidates';
  duplicateMode: 'video_rips';
  query: string;
  volumeId: number | null;
  parentId: null;
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

  return {
    mode: 'search',
    duplicateMode: 'video_rips',
    query: jump.query ?? '',
    volumeId: jump.volumeId,
    parentId: null,
    folderPrefix: jump.folderPrefix,
    category: null,
    sourceEntryId: null,
    candidateVolumeId: null,
    offset: 0,
  };
}
