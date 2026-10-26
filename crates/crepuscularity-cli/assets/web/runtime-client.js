// Request correlation only: the worker owns FIFO execution and WASM state.
export function createWorkerRuntime(worker) {
  const pending = new Map();
  const exports = new Set();
  let nextId = 1;
  let fatal = null;

  function fail(reason) {
    if (fatal) return;
    fatal = reason instanceof Error ? reason : new Error(String(reason));
    worker.removeEventListener('message', onMessage);
    worker.removeEventListener('error', onError);
    worker.removeEventListener('messageerror', onMessageError);
    for (const { reject } of pending.values()) reject(fatal);
    pending.clear();
    worker.terminate();
  }

  function onMessage({ data }) {
    const entry = pending.get(data?.id);
    if (!entry) return;
    pending.delete(data.id);
    if (Object.prototype.hasOwnProperty.call(data, 'error')) {
      entry.reject(new Error(String(data.error)));
    } else {
      entry.resolve(data.value);
    }
  }
  function onError(event) {
    fail(event.error || new Error(event.message || 'Runtime worker failed'));
  }
  function onMessageError() {
    fail(new Error('Runtime worker message could not be decoded'));
  }

  function request(fields) {
    if (fatal) return Promise.reject(fatal);
    return new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      try {
        worker.postMessage({ id, ...fields });
      } catch (error) {
        fail(error);
      }
    });
  }

  worker.addEventListener('message', onMessage);
  worker.addEventListener('error', onError);
  worker.addEventListener('messageerror', onMessageError);
  const ready = request({ kind: 'init' }).then((value) => {
    for (const name of value.exports) exports.add(name);
    return value.exports;
  });

  return {
    ready,
    has: (name) => exports.has(name),
    call: (name, ...args) => request({ kind: 'call', name, args }),
    render: (spec) => request({ kind: 'render', render: spec }),
    event: (name, eventJson, render = null) =>
      request({ kind: 'event', name, eventJson, render }),
    dispose: () => fail(new Error('Runtime worker disposed')),
  };
}
