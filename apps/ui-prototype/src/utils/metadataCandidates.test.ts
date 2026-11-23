import assert from 'node:assert/strict';
import test from 'node:test';
import {
  buildMetadataCandidateIndex,
  countCandidateVolumes,
  metadataCandidateKey,
  normalizeMetadataName,
} from './metadataCandidates';

test('candidate keys use trimmed NFC and locale-stable lowercase names', () => {
  assert.equal(normalizeMetadataName('  CAFE\u0301.MKV  '), 'café.mkv');
  assert.equal(
    metadataCandidateKey('  CAFE\u0301.MKV  ', 123),
    metadataCandidateKey('café.mkv', 123)
  );
});

test('same-size files with different names are separate metadata candidates', () => {
  const counts = countCandidateVolumes([
    { name: 'Film A.mkv', size: 10_000_000, volumeId: 1 },
    { name: 'Film B.mkv', size: 10_000_000, volumeId: 2 },
  ]);
  assert.equal(counts.get(metadataCandidateKey('Film A.mkv', 10_000_000)), 1);
  assert.equal(counts.get(metadataCandidateKey('Film B.mkv', 10_000_000)), 1);
});

test('same name and size on one disk count as one disk, despite repeated rows', () => {
  const counts = countCandidateVolumes([
    { name: 'Film.mkv', size: 10_000_000, volumeId: 1 },
    { name: 'film.mkv', size: 10_000_000, volumeId: 1 },
  ]);
  assert.equal(counts.get(metadataCandidateKey('Film.mkv', 10_000_000)), 1);
});

test('same name and size on different disks count as cross-disk candidates', () => {
  const counts = countCandidateVolumes([
    { name: 'Film.mkv', size: 10_000_000, volumeId: 1 },
    { name: 'film.mkv', size: 10_000_000, volumeId: 2 },
  ]);
  assert.equal(counts.get(metadataCandidateKey('Film.mkv', 10_000_000)), 2);
});

test('candidate index never falls back to size-only and returns one hit per disk', () => {
  const index = buildMetadataCandidateIndex([
    { name: 'Film A.mkv', size: 10_000_000, volumeId: 1, volumeLabel: 'One', relativePath: 'a/film.mkv' },
    { name: 'Film A.mkv', size: 10_000_000, volumeId: 1, volumeLabel: 'One', relativePath: 'b/film.mkv' },
    { name: 'Film A.mkv', size: 10_000_000, volumeId: 2, volumeLabel: 'Two', relativePath: 'film.mkv' },
    { name: 'Film B.mkv', size: 10_000_000, volumeId: 3, volumeLabel: 'Three', relativePath: 'film.mkv' },
  ]);

  assert.deepEqual(index.get(metadataCandidateKey('film a.MKV', 10_000_000))?.map((hit) => hit.volumeId), [1, 2]);
  assert.deepEqual(index.get(metadataCandidateKey('film b.mkv', 10_000_000))?.map((hit) => hit.volumeId), [3]);
  assert.equal(index.has(metadataCandidateKey('Film A.mkv', 10_000_001)), false);
});
