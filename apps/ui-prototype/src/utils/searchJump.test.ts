import assert from 'node:assert/strict';
import test from 'node:test';
import { buildSearchJumpDefaults } from './searchJump';

test('a normal workspace jump starts a fresh scoped search', () => {
  assert.deepEqual(
    buildSearchJumpDefaults({ volumeId: 5, folderPrefix: null }),
    {
      mode: 'search',
      duplicateMode: 'video_rips',
      query: '',
      volumeId: 5,
      parentId: null,
      folderPrefix: null,
      category: null,
      sourceEntryId: null,
      candidateVolumeId: null,
      offset: 0,
    }
  );
});

test('a copy-candidate jump clears prior search and scope context', () => {
  assert.deepEqual(
    buildSearchJumpDefaults({ volumeId: 5, folderPrefix: 'old/path', query: 'old query', copySourceEntryId: 42 }),
    {
      mode: 'copy_candidates',
      duplicateMode: 'video_rips',
      query: '',
      volumeId: null,
      parentId: null,
      folderPrefix: null,
      category: null,
      sourceEntryId: 42,
      candidateVolumeId: null,
      offset: 0,
    }
  );
});
