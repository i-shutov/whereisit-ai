import assert from 'node:assert/strict';
import test from 'node:test';
import { buildSearchJumpDefaults } from './searchJump';

test('an empty workspace jump to a known disk opens its root browse scope', () => {
  assert.deepEqual(
    buildSearchJumpDefaults({ volumeId: 5, folderPrefix: null }),
    {
      mode: 'browse',
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

test('an unknown legacy folder path stays in search mode when no stable folder id is available', () => {
  assert.equal(buildSearchJumpDefaults({ volumeId: 5, folderPrefix: 'old/path' }).mode, 'search');
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

test('a folder jump preserves its stable parent id for browse and search context', () => {
  assert.deepEqual(
    buildSearchJumpDefaults({ volumeId: 5, folderPrefix: 'old/path', folderEntryId: 91, query: 'name:term' }),
    {
      mode: 'search', duplicateMode: 'video_rips', query: 'name:term', volumeId: 5,
      parentId: 91, folderPrefix: 'old/path', category: null, sourceEntryId: null,
      candidateVolumeId: null, offset: 0,
    }
  );
});

test('copy-candidate jumps discard folder entry context as well as path context', () => {
  assert.deepEqual(
    buildSearchJumpDefaults({ volumeId: 5, folderPrefix: 'old/path', folderEntryId: 91, copySourceEntryId: 42 }),
    {
      mode: 'copy_candidates', duplicateMode: 'video_rips', query: '', volumeId: null,
      parentId: null, folderPrefix: null, category: null, sourceEntryId: 42,
      candidateVolumeId: null, offset: 0,
    }
  );
});

test('an empty jump to a known folder opens that folder in browse mode', () => {
  assert.deepEqual(
    buildSearchJumpDefaults({ volumeId: 5, folderPrefix: 'old/path', folderEntryId: 91 }),
    {
      mode: 'browse', duplicateMode: 'video_rips', query: '', volumeId: 5,
      parentId: 91, folderPrefix: 'old/path', category: null, sourceEntryId: null,
      candidateVolumeId: null, offset: 0,
    }
  );
});
