const usesPromises = globalThis.browser != null;
const nativeApi = globalThis.browser ?? globalThis.chrome;

function promisifyChrome(fn, ctx, ...args) {
  return new Promise((resolve, reject) => {
    fn.call(ctx, ...args, (result) => {
      try {
        const error = globalThis.chrome?.runtime?.lastError;
        if (error) {
          reject(new Error(error.message));
          return;
        }
        resolve(result);
      } catch (error) {
        reject(error);
      }
    });
  });
}

function storageArea(area) {
  const api = nativeApi.storage[area];
  return {
    async get(keys) {
      if (usesPromises) {
        return api.get(keys);
      }
      return promisifyChrome(api.get, api, keys);
    },
    async set(value) {
      if (usesPromises) {
        return api.set(value);
      }
      return promisifyChrome(api.set, api, value);
    }
  };
}

export const browserApi = {
  runtime: {
    getURL: (path) => nativeApi.runtime.getURL(path),
    onMessage: nativeApi.runtime.onMessage,
    async sendMessage(payload) {
      if (usesPromises) {
        return nativeApi.runtime.sendMessage(payload);
      }
      return promisifyChrome(nativeApi.runtime.sendMessage, nativeApi.runtime, payload);
    }
  },
  storage: {
    local: storageArea("local"),
    sync: storageArea("sync")
  }
};
