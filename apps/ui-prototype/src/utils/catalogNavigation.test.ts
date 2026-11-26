import assert from 'node:assert/strict';
import test from 'node:test';
import { breadcrumbPath, destinationForEmptyQuery, folderChildrenOnPage, includeFocusedFolder } from './catalogNavigation';
import type { CatalogEntry } from '../types/atlas';

test('clearing a query returns to the remembered disk and folder, or catalog overview', () => {
  assert.deepEqual(destinationForEmptyQuery(12, 34), { view: 'browse', volumeId: 12, parentId: 34 });
  assert.deepEqual(destinationForEmptyQuery(null, null), { view: 'overview', volumeId: null, parentId: null });
});

test('catalog breadcrumb targets retain parent ids and truncate deeper folders', () => {
  const crumbs = [{ id: 3, name: 'Archive' }, { id: 8, name: 'Films' }, { id: 21, name: 'Drama' }];
  assert.deepEqual(breadcrumbPath(crumbs, 1), { parentId: 8, crumbs: crumbs.slice(0, 2) });
  assert.deepEqual(breadcrumbPath(crumbs, -1), { parentId: null, crumbs: [] });
});

test('tree pagination exposes only directories from each page without implying completion', () => {
  const entry = (id: number, kind: CatalogEntry['kind']): CatalogEntry => ({
    id, volumeId: 1, volumeLabel: 'Synthetic disk', parentId: null, name: `item-${id}`, nameNorm: `item-${id}`,
    relativePath: `item-${id}`, ext: '', kind, size: 0, modifiedUnix: 0, category: '', description: '', tags: '', duplicateCount: 1,
  });
  const folders = folderChildrenOnPage([entry(1, 'directory'), entry(2, 'file'), entry(3, 'directory')]);
  assert.deepEqual(folders.map((folder) => folder.id), [1, 3]);
  // Whether more pages exist is maintained separately by the API response; filtering a page never marks it complete.
  assert.equal(folders.length, 2);
});

test('focused ancestor remains visible beyond the first 250 sibling entries without marking the page complete', () => {
  const page = Array.from({ length: 250 }, (_, index) => ({
    id: index + 1, name: `folder-${index + 1}`, volumeId: 4, parentId: 10,
  }));
  const focused = { id: 320, name: 'focused-folder', volumeId: 4, parentId: 10 };
  const merged = includeFocusedFolder(page, focused);
  assert.equal(merged.length, 251);
  assert.equal(merged.find((folder) => folder.id === 320)?.name, 'focused-folder');
  assert.equal(includeFocusedFolder(merged, focused).length, 251);
});
