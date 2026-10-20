// Actual app source + happy-dom; transport is a labelled stub, not browser/WASM proof.
import { afterEach, beforeEach, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { GlobalRegistrator } from '@happy-dom/global-registrator';

const source = readFileSync(`${import.meta.dir}/../assets/web/app.js`, 'utf8')
  .replaceAll('import.meta.url', JSON.stringify('http://localhost/app.js'));
const subsystem = source.slice(source.indexOf('const delegatedEvents'), source.indexOf('// ── Bootstrap'));
let api: any;
let root: HTMLElement;
let requests: any[];
let mainStarts: number;
let workerStarts: number;

beforeEach(() => {
  GlobalRegistrator.register();
  document.body.innerHTML = '<main id="root"><input value="old"></main>';
  root = document.getElementById('root')!;
  requests = [];
  mainStarts = workerStarts = 0;
  const transport = {
    ready: Promise.resolve(),
    render(spec: any) { return enqueue({ kind: 'render', render: spec }); },
    event(name: string, eventJson: string, render: any) { return enqueue({ kind: 'event', name, eventJson, render }); },
  };
  function enqueue(fields: any) {
    return new Promise(resolve => requests.push({ ...fields, resolve }));
  }
  api = new Function('runtime', 'init', 'createWorkerRuntime', 'Worker', 'transport', `${subsystem}
    return { renderInWorker, initializeRuntime,
      unmark() { delete runtime.crepus_worker_runtime; },
      configure(root, bundle) { currentRoot = root; currentBundle = bundle; workerRuntime = transport; },
      revise(bundle) { currentBundle = bundle; bundleRevision++; },
      initialize() { workerRuntime = null; return initializeRuntime(); }
    };`)(
      { crepus_worker_runtime() {}, render_card() {} },
      () => { mainStarts++; return Promise.resolve(); },
      (worker: any) => { if (worker) workerStarts++; return transport; },
      class WorkerStub {},
      transport,
    );
  api.configure(root, { entry: 'old', files: {} });
});
afterEach(() => GlobalRegistrator.unregister());

test('newer root render wins over a late older response', async () => {
  const old = api.renderInWorker();
  const fresh = api.renderInWorker();
  requests[1].resolve('<p>new</p>'); await fresh;
  requests[0].resolve('<p>old</p>'); await old;
  expect(root.textContent).toBe('new');
});

test('bundle revision invalidates an outstanding root result', async () => {
  const pending = api.renderInWorker();
  api.revise({ entry: 'new', files: {} });
  requests[0].resolve('<p>obsolete</p>'); await pending;
  expect(root.querySelector('input')).not.toBeNull();
  const next = api.renderInWorker();
  expect(requests[1].render.args[0].entry).toBe('new');
  requests[1].resolve('<p>current</p>'); await next;
  expect(root.textContent).toBe('current');
});

test('scope generation and connectivity guard scoped commits', async () => {
  root.innerHTML = '<section data-crepus-island-props="{&quot;title&quot;:&quot;雪&quot;}"></section>';
  const scope = root.firstElementChild!;
  const old = api.renderInWorker(scope, 'card');
  const fresh = api.renderInWorker(scope, 'card');
  expect(requests[0].render).toEqual({ name: 'render_card', args: [{ title: '雪' }], json: true });
  requests[1].resolve('new'); await fresh;
  requests[0].resolve('old'); await old;
  expect(scope.textContent).toBe('new');
  const detached = api.renderInWorker(scope, 'card');
  scope.remove();
  requests[2].resolve('detached'); await detached;
  expect(scope.textContent).toBe('new');
});

test('event and structured render travel together; composing events skip commits', async () => {
  const event = api.renderInWorker(null, null, 'clicked', { value: '雪' });
  expect(requests[0].kind).toBe('event');
  expect(requests[0].name).toBe('clicked');
  expect(JSON.parse(requests[0].eventJson)).toEqual({ value: '雪' });
  expect(requests[0].render).toEqual({ name: 'crepus_render', args: [{ entry: 'old', files: {} }], json: true });
  requests[0].resolve('<p>event</p>'); await event;
  const composing = api.renderInWorker(null, null, 'typed', { value: 'draft' }, true);
  expect(requests[1].render).toBeNull();
  requests[1].resolve('<p>must not commit</p>'); await composing;
  expect(root.textContent).toBe('event');
});

test('focus and latest selection are restored at response commit', async () => {
  const input = root.querySelector('input')!;
  input.value = 'abcdef'; input.focus(); input.setSelectionRange(1, 2);
  const pending = api.renderInWorker();
  input.setSelectionRange(3, 5, 'backward');
  requests[0].resolve('<input value="abcdef">'); await pending;
  const replacement = root.querySelector('input')!;
  expect(document.activeElement).toBe(replacement);
  expect(replacement.selectionStart).toBe(3);
  expect(replacement.selectionEnd).toBe(5);
  expect(replacement.selectionDirection).toBe('backward');
});

test('default marker selects worker while explicit main keeps main initialization', async () => {
  await api.initialize();
  expect(workerStarts).toBe(1); expect(mainStarts).toBe(0);
  document.head.innerHTML = '<meta name="crepus-runtime-thread" content="main">';
  await api.initialize();
  expect(workerStarts).toBe(1); expect(mainStarts).toBe(1);
});

test('scoped event detached by an earlier root commit refreshes root without replaying handler', async () => {
  root.innerHTML = '<section data-crepus-scope="card">old</section>';
  const oldScope = root.firstElementChild!;
  const rootRender = api.renderInWorker();
  const transaction = api.renderInWorker(oldScope, 'card', 'increment', { value: 1 });
  expect(requests[1].kind).toBe('event');
  requests[0].resolve('<section data-crepus-scope="card">before event</section>');
  await rootRender;
  expect(oldScope.isConnected).toBe(false);
  requests[1].resolve('scoped state after event');
  // Let the scoped transaction request its replacement root snapshot.
  await Promise.resolve();
  expect(requests).toHaveLength(3);
  expect(requests[2].kind).toBe('render');
  expect(requests[2].render.name).toBe('crepus_render');
  expect(requests.filter(request => request.kind === 'event')).toHaveLength(1);
  requests[2].resolve('<section data-crepus-scope="card">after event</section>');
  await transaction;
  expect(root.textContent).toBe('after event');
});


test('unmarked legacy runtime stays on main unless explicitly opted in', async () => {
  api.unmark();
  await api.initialize();
  expect(mainStarts).toBe(1); expect(workerStarts).toBe(0);
  document.head.innerHTML = '<meta name="crepus-runtime-thread" content="worker">';
  await api.initialize();
  expect(mainStarts).toBe(1); expect(workerStarts).toBe(1);
});

test('invalid runtime thread mode reports a configuration error', () => {
  document.head.innerHTML = '<meta name="crepus-runtime-thread" content="unknown">';
  expect(() => api.initialize()).toThrow('crepus-runtime-thread must be main or worker');
  expect(mainStarts).toBe(0); expect(workerStarts).toBe(0);
});
