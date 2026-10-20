import { expect, test } from 'bun:test';
import { createWorkerRuntime } from '../assets/web/runtime-client.js';

class FakeWorker extends EventTarget {
  sent: any[] = [];
  terminated = 0;
  postFailure: Error | null = null;
  postMessage(message: any) {
    if (this.postFailure) throw this.postFailure;
    this.sent.push(message);
  }
  terminate() { this.terminated++; }
  reply(id: number, fields: object) {
    this.dispatchEvent(new MessageEvent('message', { data: { id, ...fields } }));
  }
}

function initialized() {
  const worker = new FakeWorker();
  const runtime = createWorkerRuntime(worker);
  worker.reply(1, { value: { exports: ['render', 'click'] } });
  return { worker, runtime };
}

test('init discovers exports; pending calls correlate independently', async () => {
  const worker = new FakeWorker();
  const runtime = createWorkerRuntime(worker);
  expect(runtime.has('render')).toBe(false);
  expect(worker.sent).toEqual([{ id: 1, kind: 'init' }]);
  worker.reply(1, { value: { exports: ['render'] } });
  expect(await runtime.ready).toEqual(['render']);
  expect(runtime.has('render')).toBe(true);
  expect(runtime.has('missing')).toBe(false);
  const first = runtime.call('render', 'one');
  const second = runtime.call('render', 'two');
  worker.reply(999, { value: 'ignored' });
  worker.reply(3, { value: 'second' });
  worker.reply(2, { value: 'first' });
  expect(await first).toBe('first');
  expect(await second).toBe('second');
  runtime.dispose();
});

test('event forwards event and render together; request errors do not kill transport', async () => {
  const { worker, runtime } = initialized();
  await runtime.ready;
  const event = runtime.event('click', '{"value":1}', { name: 'render', args: ['props'] });
  expect(worker.sent[1]).toEqual({ id: 2, kind: 'event', name: 'click', eventJson: '{"value":1}', render: { name: 'render', args: ['props'] } });
  worker.reply(2, { error: 'handler failed' });
  await expect(event).rejects.toThrow('handler failed');
  const next = runtime.event('click', '{}');
  expect(worker.sent[2].render).toBeNull();
  worker.reply(3, { value: null });
  expect(await next).toBeNull();
  expect(worker.terminated).toBe(0);
  runtime.dispose();
});

for (const kind of ['error', 'messageerror', 'dispose']) {
  test(`${kind} rejects every pending request and future calls, terminates once`, async () => {
    const { worker, runtime } = initialized();
    await runtime.ready;
    const first = runtime.call('render');
    const second = runtime.call('click');
    const outcomes = Promise.allSettled([first, second]);
    if (kind === 'dispose') runtime.dispose();
    else worker.dispatchEvent(new Event(kind));
    expect((await outcomes).map(result => result.status)).toEqual(['rejected', 'rejected']);
    await expect(runtime.call('later')).rejects.toThrow();
    runtime.dispose();
    worker.dispatchEvent(new Event('error'));
    expect(worker.terminated).toBe(1);
    expect(worker.sent).toHaveLength(3);
  });
}

test('failed postMessage rejects prior and current pending requests without replay', async () => {
  const { worker, runtime } = initialized();
  await runtime.ready;
  const earlier = runtime.call('render');
  worker.postFailure = new Error('cannot clone');
  const outcomes = Promise.allSettled([earlier, runtime.call('click', () => {})]);
  for (const result of await outcomes) {
    expect(result.status).toBe('rejected');
    if (result.status === 'rejected') expect(result.reason.message).toBe('cannot clone');
  }
  await expect(runtime.call('later')).rejects.toThrow('cannot clone');
  worker.reply(2, { value: 'late' });
  runtime.dispose();
  expect(worker.terminated).toBe(1);
  expect(worker.sent).toHaveLength(2);
});

test('failure while initializing rejects ready', async () => {
  const worker = new FakeWorker();
  const runtime = createWorkerRuntime(worker);
  worker.dispatchEvent(new Event('messageerror'));
  await expect(runtime.ready).rejects.toThrow('decoded');
  expect(runtime.has('render')).toBe(false);
  expect(worker.terminated).toBe(1);
});

// Structured bundle data stays structured until the worker serializes it.
test('render forwards the JSON serialization flag without stringifying on the caller', async () => {
  const { worker, runtime } = initialized();
  await runtime.ready;
  const bundle = { entry: 'index.crepus', files: { 'index.crepus': 'span "雪"' } };
  const result = runtime.render({ name: 'crepus_render', args: [bundle], json: true });
  expect(worker.sent[1]).toEqual({ id: 2, kind: 'render', render: { name: 'crepus_render', args: [bundle], json: true } });
  expect(worker.sent[1].render.args[0]).toBe(bundle);
  worker.reply(2, { value: '<span>雪</span>' });
  expect(await result).toBe('<span>雪</span>');
  runtime.dispose();
});
