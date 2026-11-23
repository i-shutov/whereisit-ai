import assert from 'node:assert/strict';
import test from 'node:test';
import { ScopedRequestGate } from './scopedRequest';

test('failed requests can be retried for the same scope', () => {
  const gate = new ScopedRequestGate();
  const failed = gate.begin('volume-1')!;

  assert.equal(gate.fail(failed), true);
  const retry = gate.begin('volume-1');

  assert.ok(retry);
  assert.notEqual(retry.sequence, failed.sequence);
  assert.equal(gate.succeed(retry), true);
  assert.equal(gate.begin('volume-1'), null);
});

test('a late response from an older scope cannot replace the newer result', () => {
  const gate = new ScopedRequestGate();
  const oldScope = gate.begin('volume-1')!;
  const currentScope = gate.begin('volume-2')!;

  assert.equal(gate.succeed(oldScope), false);
  assert.equal(gate.isCurrent(currentScope), true);
  assert.equal(gate.succeed(currentScope), true);
  assert.equal(gate.begin('volume-2'), null);
  assert.ok(gate.begin('volume-1'));
});

test('returning to a cached scope invalidates a different pending request without refetching', () => {
  const gate = new ScopedRequestGate();
  const scopeA = gate.begin('scope-a')!;
  assert.equal(gate.succeed(scopeA), true);

  const scopeB = gate.begin('scope-b')!;
  assert.equal(gate.isCurrent(scopeB), true);

  assert.equal(gate.begin('scope-a'), null);
  assert.equal(gate.isCurrent(scopeB), false);
  assert.equal(gate.succeed(scopeB), false);
  assert.ok(gate.begin('scope-b'));
});

test('leaving a workspace invalidates its pending request and allows retry on return', () => {
  const gate = new ScopedRequestGate();
  const pending = gate.begin('generation-3')!;

  gate.invalidate(pending);

  assert.equal(gate.isCurrent(pending), false);
  assert.ok(gate.begin('generation-3'));
});
