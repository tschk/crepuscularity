import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const source = readFileSync(`${import.meta.dir}/../assets/web/app.js`, "utf8");
const bootstrap = source.slice(source.indexOf("async function main()")).replace("main().catch(", "const completed = main().catch(");

async function run(mode: string) {
  // Isolate unhandled-rejection monitoring from Bun's test process. Execute the
  // real bootstrap, replacing only its browser/runtime/network boundaries.
  const script = `
import { Window } from 'happy-dom';
const window=new Window();const document=window.document;
const mode=${JSON.stringify(mode)};
const unhandled=[];process.on('unhandledRejection',error=>unhandled.push(String(error)));
const errors=[];const console={error:error=>errors.push(String(error))};
document.body.innerHTML='<div id="crepus-root">'+(mode==='ssr'?'<p data-crepus-id="0">SSR</p>':'')+'</div>';
let currentBundle,currentIslandManifest,currentRoot;
let disposed=0;const workerRuntime={dispose(){disposed++}};
function deferred(){let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no});return{promise,resolve,reject}}
const readiness=deferred(),bundleFetch=deferred(),manifestFetch=deferred();
let settled=false;
const ready=readiness.promise;
function resolveReady(){settled=true;readiness.resolve()}
function initializeRuntime(){return ready}
const fetch=()=>bundleFetch.promise;
async function loadIslandManifest(){await manifestFetch.promise;return{islands:{}}}
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
const bundleResponse={ok:true,json:async()=>({files:{},entry:'index.crepus'})};
let delegatedBeforeReady=false,interactive=0;
function initDelegatedEvents(root,received){delegatedBeforeReady=received===ready&&mode==='ssr'&&!settled}
function initInteractive(){interactive++}
function setRootHtml(root,html){root.innerHTML=html}
const runtime={crepus_render:()=>'<p>Rendered</p>'};
async function renderInWorker(){setRootHtml(currentRoot,'<p>Rendered</p>')}
${bootstrap}
if(mode==='reject'){
 readiness.reject(new Error('early init failure'));
 await tick();
 bundleFetch.resolve(bundleResponse);
 await tick();
 manifestFetch.resolve();
}else if(mode==='fetch-fail'){
 resolveReady();
 bundleFetch.reject(new Error('bundle fetch failed'));
}else{
 bundleFetch.resolve(bundleResponse);
 await tick();
 manifestFetch.resolve();
 await tick();
 if(!delegatedBeforeReady)throw new Error('SSR delegation did not precede readiness');
 resolveReady();
}
await completed;
await tick();
process.stdout.write(JSON.stringify({unhandled,errors,disposed,delegatedBeforeReady,interactive,text:document.body.textContent}));
window.happyDOM.abort();
`;
  const child = Bun.spawn([process.execPath, "--eval", script], {
    cwd: `${import.meta.dir}/../../..`, stdout: "pipe", stderr: "pipe",
  });
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
  ]);
  expect(code, stderr).toBe(0);
  return JSON.parse(stdout);
}

test("early readiness failure is handled while bundle and manifest fetches await", async () => {
  const result = await run("reject");
  expect(result.unhandled).toEqual([]);
  expect(result.errors).toEqual(["Error: early init failure"]);
  expect(result.disposed).toBe(1);
  expect(result.text).toContain("WASM boot failed");
  expect(result.text).toContain("early init failure");
  expect(result.interactive).toBe(0);
});

test("bundle-fetch failure disposes a successfully initialized worker", async () => {
  const result = await run("fetch-fail");
  expect(result.unhandled).toEqual([]);
  expect(result.errors).toEqual(["Error: bundle fetch failed"]);
  expect(result.disposed).toBe(1);
  expect(result.text).toContain("bundle fetch failed");
});

test("SSR delegation receives original readiness promise and retains existing markup", async () => {
  const result = await run("ssr");
  expect(result.unhandled).toEqual([]);
  expect(result.errors).toEqual([]);
  expect(result.disposed).toBe(0);
  expect(result.delegatedBeforeReady).toBe(true);
  expect(result.interactive).toBe(1);
  expect(result.text).toBe("SSR");
});
