// P2#9：worker 线协议归一的单测（零依赖，用 Node 内置 test runner：`node --test`）。
// 守住 loader 封套字段（full / conversation_id）→ 线协议字段（full_text / native_conversation_id）的映射，
// 防止跨层重命名漂移（[35]）。需 Node ≥ 22（TS 类型擦除）；构建仍走 Node 18 / vite，互不影响。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { toWireChunk, toWireDone, toWireError } from './workerProtocol.ts';

test('toWireChunk passes req_id + delta，缺 delta 补空串', () => {
  assert.deepEqual(toWireChunk({ req_id: 'r1', delta: 'hi' }), { req_id: 'r1', delta: 'hi' });
  assert.deepEqual(toWireChunk({ req_id: 'r1' }), { req_id: 'r1', delta: '' });
});

test('toWireDone 把 full→full_text、conversation_id→native_conversation_id', () => {
  assert.deepEqual(
    toWireDone({ req_id: 'r2', full: 'answer', conversation_id: 'c-9' }),
    { req_id: 'r2', full_text: 'answer', native_conversation_id: 'c-9' }
  );
  // 缺省：full_text 空串、native_conversation_id 归一为 null（而非 undefined）。
  assert.deepEqual(toWireDone({ req_id: 'r2' }), {
    req_id: 'r2',
    full_text: '',
    native_conversation_id: null,
  });
});

test('toWireError 默认 code=content_error，缺 message 补空串', () => {
  assert.deepEqual(toWireError({ req_id: 'r3', message: 'boom' }), {
    req_id: 'r3',
    code: 'content_error',
    message: 'boom',
    cooling_until: 0,
  });
  assert.deepEqual(toWireError({ req_id: 'r3' }, 'image_failed'), {
    req_id: 'r3',
    code: 'image_failed',
    message: '',
    cooling_until: 0,
  });
});
