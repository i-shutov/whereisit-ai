import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyExternalDemoChange, COPY_DEMO_DOCUMENT_ID, demoDocumentKey, demoRecordHasConflict, initialDemoRecord, markDemoRecordSaved, parseDemoRecord, PRIMARY_DEMO_DOCUMENT_ID, resolveDemoConflict } from './demoAnnotations';

const original = { description: 'Original', tags: 'film,4k', category: 'Cinema' };

test('synthetic record detects an external description change and requires an explicit choice', () => {
  const local = { ...initialDemoRecord(original), value: { ...original, description: 'My note' } };
  const conflicted = applyExternalDemoChange(local, 'Edited in WhereIsIt demo');
  assert.equal(demoRecordHasConflict(conflicted), true);
  assert.equal(conflicted.value.description, 'My note');
  assert.equal(conflicted.external?.description, 'Edited in WhereIsIt demo');
  assert.equal(resolveDemoConflict(conflicted, 'mine').value.description, 'My note');
  assert.equal(resolveDemoConflict(conflicted, 'external').value.description, 'Edited in WhereIsIt demo');
});

test('byte-identical mock copies have separate synthetic document identities', () => {
  assert.notEqual(demoDocumentKey(PRIMARY_DEMO_DOCUMENT_ID, 1, 41), demoDocumentKey(COPY_DEMO_DOCUMENT_ID, 1, 41));
  assert.notEqual(demoDocumentKey(PRIMARY_DEMO_DOCUMENT_ID, 1, 41), demoDocumentKey(PRIMARY_DEMO_DOCUMENT_ID, 2, 41));
});

test('saving a local overlay preserves the source baseline for the next external conflict', () => {
  const local = { ...initialDemoRecord(original), value: { ...original, description: 'My note', tags: 'my-tag' } };
  const saved = markDemoRecordSaved(local);
  assert.equal(saved.base.description, 'Original');
  const changed = applyExternalDemoChange(saved, 'External description');
  assert.equal(demoRecordHasConflict(changed), true);
  const useExternal = resolveDemoConflict(changed, 'external');
  assert.equal(useExternal.value.description, 'External description');
  assert.equal(useExternal.value.tags, 'my-tag');
  assert.equal(useExternal.base.description, 'External description');
  const keepMine = resolveDemoConflict(changed, 'mine');
  assert.equal(keepMine.value.description, 'My note');
  assert.equal(keepMine.value.tags, 'my-tag');
});

test('external description changes merge without conflicting with local tags and category', () => {
  const local = { ...initialDemoRecord(original), value: { ...original, tags: 'my-tag', category: 'My category' } };
  const changed = applyExternalDemoChange(local, 'External description');
  assert.equal(demoRecordHasConflict(changed), false);
  assert.equal(changed.value.description, 'External description');
  const accepted = resolveDemoConflict(changed, 'external');
  assert.equal(accepted.value.tags, 'my-tag');
  assert.equal(accepted.value.category, 'My category');
});

test('an unambiguous external merge advances baseline and later distinct edit conflicts normally', () => {
  const local = { ...initialDemoRecord(original), value: { ...original, tags: 'my-tag' } };
  const accepted = applyExternalDemoChange(local, 'External generation one');
  assert.equal(accepted.external, null);
  assert.equal(accepted.base.description, 'External generation one');
  assert.equal(accepted.value.tags, 'my-tag');
  const laterLocal = { ...accepted, value: { ...accepted.value, description: 'My later note' } };
  assert.equal(demoRecordHasConflict(laterLocal), false);
  const nextExternal = applyExternalDemoChange(laterLocal, 'External generation two');
  assert.equal(demoRecordHasConflict(nextExternal), true);
  assert.equal(nextExternal.external?.description, 'External generation two');
});

test('malformed browser records are rejected instead of being normalized over', () => {
  assert.throws(() => parseDemoRecord('{"base":{},"value":{},"external":null}'));
});
