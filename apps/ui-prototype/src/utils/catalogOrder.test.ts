import assert from 'node:assert/strict';
import test from 'node:test';
import { orderSearchMatches } from './catalogOrder';

test('catalog order keeps filtered synthetic matches in source order across pagination', () => {
  const source = [
    { id: 41, kind: 'file' as const, size: 3, nameNorm: 'zeta' },
    { id: 12, kind: 'file' as const, size: 90, nameNorm: 'alpha' },
    { id: 57, kind: 'directory' as const, size: 0, nameNorm: 'folder' },
  ];
  const filtered = source.filter((entry) => entry.id !== 57);

  assert.deepEqual(orderSearchMatches(filtered, 'catalog').slice(0, 1).map((entry) => entry.id), [41]);
  assert.deepEqual(orderSearchMatches(source.filter((entry) => entry.id !== 57)).slice(0, 1).map((entry) => entry.id), [12]);
});
