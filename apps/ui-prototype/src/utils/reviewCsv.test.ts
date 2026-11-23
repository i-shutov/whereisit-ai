import assert from 'node:assert/strict';
import test from 'node:test';
import { escapeReviewCsvCell } from './reviewCsv';

test('review CSV preserves quotes and embedded line breaks', () => {
  assert.equal(escapeReviewCsvCell('folder/line\nbreak "name".txt'), '"folder/line\nbreak ""name"".txt"');
});

test('review CSV neutralizes spreadsheet formulas after leading whitespace', () => {
  assert.equal(escapeReviewCsvCell('  =1+1'), '"\'  =1+1"');
  assert.equal(escapeReviewCsvCell('@SUM(A1:A2)'), '"\'@SUM(A1:A2)"');
});
