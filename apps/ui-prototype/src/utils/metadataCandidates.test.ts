import assert from 'node:assert/strict';
import test from 'node:test';
import { countCandidateVolumes, metadataCandidateKey } from './metadataCandidates';

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
