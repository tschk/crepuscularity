#!/usr/bin/env bun
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

type Manifest = { name: string; version: string; private?: boolean; dependencies?: Record<string,string>; optionalDependencies?: Record<string,string>; peerDependencies?: Record<string,string>; peerDependenciesMeta?: Record<string,{optional?:boolean}> };
type Entry = { name: string; version?: string; path?: string; artifact?: string; sha512?: string; observedLatest: string; releaseNote?: string };
type Registry = { name: string; maintainers?: {name:string}[]; versions: Record<string,Manifest & {dist?:{tarball?:string}}> ; 'dist-tags': Record<string,string> };
type Candidate = { entry: Entry; manifest: Manifest; dir?: string; artifact?: string; registry: Registry };
const base = import.meta.dir;
const registryURL = 'https://registry.npmjs.org';

function run(args: string[], cwd = base, inherit = false): string {
  const proc = Bun.spawnSync(args, { cwd, stdin: inherit ? 'inherit' : 'ignore', stdout: inherit ? 'inherit' : 'pipe', stderr: inherit ? 'inherit' : 'pipe' });
  if (proc.exitCode !== 0) throw new Error(`${args[0]} ${args[1] ?? ''} failed (${proc.exitCode}).${inherit ? '' : '\n'+proc.stderr.toString().slice(-3000)}`);
  return inherit ? '' : proc.stdout.toString().trim();
}
async function metadata(name: string): Promise<Registry> {
  const res = await fetch(`${registryURL}/${encodeURIComponent(name)}`, {signal:AbortSignal.timeout(30000)});
  if (!res.ok) throw new Error(`${name}: registry HTTP ${res.status}; cannot treat this as an unpublished version.`);
  return await res.json() as Registry;
}
export function status(version: string | undefined, info: Registry): string {
  if (!version) return 'NO LOCAL SOURCE';
  if (Object.hasOwn(info.versions, version)) return 'CURRENT';
  const latest = info['dist-tags'].latest;
  if (latest && Bun.semver.order(version, latest) <= 0) return 'OLDER LOCAL';
  return 'PENDING';
}
export function ordered(packages: Candidate[]): Candidate[] {
  const map = new Map(packages.map(p=>[p.manifest.name,p]));
  const active=new Set<string>(), seen=new Set<string>(), result:Candidate[]=[];
  function visit(p:Candidate) {
    if (seen.has(p.manifest.name)) return;
    if (active.has(p.manifest.name)) throw new Error(`Required dependency cycle at ${p.manifest.name}`);
    active.add(p.manifest.name);
    const deps = {...p.manifest.dependencies,...p.manifest.optionalDependencies};
    for (const [n,r] of Object.entries(p.manifest.peerDependencies ?? {})) if (!p.manifest.peerDependenciesMeta?.[n]?.optional) deps[n]=r;
    for (const [name,range] of Object.entries(deps)) {
      const dep=map.get(name);
      if (!dep) continue;
      if (!Bun.semver.satisfies(dep.manifest.version, range)) throw new Error(`${p.manifest.name}: ${name}@${range} does not accept selected ${dep.manifest.version}`);
      visit(dep);
    }
    active.delete(p.manifest.name); seen.add(p.manifest.name); result.push(p);
  }
  for (const p of packages) visit(p);
  return result;
}
export function validatePacked(manifest: Manifest) {
  if (manifest.private === true) throw new Error(`${manifest.name} is private`);
  for (const section of [manifest.dependencies,manifest.optionalDependencies,manifest.peerDependencies]) {
    for (const [name,range] of Object.entries(section ?? {})) {
      if (/^(file:|link:|workspace:|catalog:|\.\.?\/|\/)/.test(range)) throw new Error(`${manifest.name}: unpackable local dependency ${name}@${range}`);
    }
  }
}
export async function assertDependencies(manifest:Manifest, prepared:Map<string,Manifest>, lookup:(name:string)=>Promise<string[]>) {
  const deps={...manifest.dependencies,...manifest.optionalDependencies};
  for(const [name,range]of Object.entries(manifest.peerDependencies??{}))if(!manifest.peerDependenciesMeta?.[name]?.optional)deps[name]=range;
  for(const [name,range]of Object.entries(deps)) {
    const selected=prepared.get(name);
    if(selected && Bun.semver.satisfies(selected.version,range))continue;
    const versions=await lookup(name);
    if(!versions.some(v=>Bun.semver.satisfies(v,range)))throw new Error(`${manifest.name}: required ${name}@${range} is neither available on npm nor prepared in this run`);
  }
}
function inspectArtifact(file:string, expected:Manifest): Manifest {
  const listing=run(['tar','-tzf',file]).split('\n');
  if (listing.some(n=>!n.startsWith('package/') || n.split('/').includes('..') || /(^|\/)(\.npmrc|\.env(?:\..*)?|id_rsa|id_ed25519)$/.test(n))) throw new Error(`Unexpected or credential-like path in ${file}`);
  const packed=JSON.parse(run(['tar','-xOf',file,'package/package.json'])) as Manifest;
  if (packed.name!==expected.name || packed.version!==expected.version) throw new Error(`Tarball identity differs for ${expected.name}`);
  validatePacked(packed);
  return packed;
}
async function checkPublished(info:Registry, version:string) {
  const url=info.versions[version]?.dist?.tarball;
  if (!url || new URL(url).origin!==registryURL) throw new Error(`${info.name}@${version}: missing/unexpected artifact URL`);
  const res=await fetch(url,{method:'HEAD',signal:AbortSignal.timeout(30000)});
  if (!res.ok) throw new Error(`${info.name}@${version}: registered tarball HTTP ${res.status}; bump the version after investigation, do not overwrite it.`);
}
function cleanSource(dir:string):string {
  const root=run(['git','rev-parse','--show-toplevel'],dir);
  if (run(['git','status','--porcelain','--untracked-files=normal'],root)) throw new Error(`${root}: uncommitted changes; finish/review the release source first. No files were reverted.`);
  return root;
}
function checkRepo(root:string, projects:string, checked:Set<string>) {
  if (checked.has(root)) return;
  console.log(`Checks: ${root}`);
  if (root===join(projects,'moonshine')) {
    for (const script of ['test','typecheck','typecheck:native','check:packages','check:catalog','check:adapters']) run(['bun','run',script],root,true);
  } else if ([join(projects,'minion-workspace/work/flowtoken-svelte'),join(projects,'minion-workspace/work/svelte-streamdown')].includes(root)) {
    const packageDir=join(root,'packages',root.endsWith('/flowtoken-svelte')?'flowtoken-svelte':'svelte-streamdown');
    for(const script of ['check-types','test','build'])run(['bun','run',script],packageDir,true);
  } else {
    throw new Error(`${root}: no release validation recipe; add one before publishing a new version.`);
  }
  cleanSource(root); checked.add(root);
}
async function main() {
  const args=process.argv.slice(2);
  if (args.includes('--help')) {
    console.log('bun publish.ts [--login] [--prepare | --publish] [--only NAME_SUBSTRING] [--projects DIR]\nDefault: live read-only public-package inventory. --prepare checks/packs; --publish also uploads.\n--login uses npm browser authentication; uploads use Bun. Existing versions are skipped.');return;
  }
  let publish=false,prepare=false,login=false,only='',projects=join(homedir(),'projects');
  for(let i=0;i<args.length;i++) {
    const a=args[i];
    if(a==='--publish') publish=true;
    else if(a==='--prepare') prepare=true;
    else if(a==='--login') login=true;
    else if(a==='--only' || a==='--projects') { const value=args[++i];if(!value || value.startsWith('--'))throw new Error(`${a} needs a value`);if(a==='--only')only=value;else projects=resolve(value); }
    else throw new Error(`Unknown argument ${a}`);
  }
  if(publish&&prepare)throw new Error('Choose --prepare or --publish');
  const config=JSON.parse(readFileSync(join(base,'release.json'),'utf8')) as {owner:string;packages:Entry[]};
  if(login) run(['npm','login','--registry',registryURL,'--auth-type=web'],base,true);
  if(publish) {
    const who=run(['bun','pm','whoami','--registry',registryURL]);
    if(who!==config.owner)throw new Error(`Expected npm user ${config.owner}; received ${who}. Run with --login and retry; never paste a token.`);
  }
  const entries=config.packages.filter(e=>e.name.includes(only));
  if(!entries.length)throw new Error('No packages match --only');
  const candidates:Candidate[]=[];
  // Limit registry concurrency and fail closed on HTTP/auth/network errors.
  for(let start=0;start<entries.length;start+=5) {
    const rows=await Promise.all(entries.slice(start,start+5).map(async entry=>{
      const info=await metadata(entry.name);
      if(!info.maintainers?.some(m=>m.name===config.owner))throw new Error(`${entry.name}: public maintainers no longer include ${config.owner}`);
      let manifest:Manifest|undefined,dir:string|undefined;
      if(entry.path) {dir=join(projects,entry.path);const f=join(dir,'package.json');if(existsSync(f))manifest=JSON.parse(readFileSync(f,'utf8'));}
      else if(entry.artifact) manifest={name:entry.name,version:entry.version!};
      if(manifest && manifest.name!==entry.name)throw new Error(`Local package identity changed at ${dir}`);
      const state=manifest?.private ? 'PRIVATE' : status(manifest?.version,info);
      console.log(`${state.padEnd(16)} ${entry.name}  npm=${info['dist-tags'].latest ?? '(none)'} local=${manifest?.version ?? '(none)'}`);
      if(state==='CURRENT') {await checkPublished(info,manifest!.version);if(entry.releaseNote)console.log(`  NOTE ${entry.releaseNote}`);}
      return state==='PENDING' ? {entry,manifest:manifest!,dir,registry:info} : null;
    }));
    for(const row of rows)if(row)candidates.push(row);
  }
  console.log(`\n${candidates.length} unpublished local versions. A version difference alone is not a passing release check.`);
  if(!publish&&!prepare)return;
  const checked=new Set<string>(), ready:Candidate[]=[], blocked:string[]=[];
  const packDir=join(base,'prepared');mkdirSync(packDir,{recursive:true});
  for(const candidate of ordered(candidates)) {
    const {entry,manifest,dir}=candidate;
    try {
      let artifact:string;
      if(entry.artifact) {
        artifact=join(base,entry.artifact);
        const hash=createHash('sha512').update(readFileSync(artifact)).digest('hex');
        if(hash!==entry.sha512)throw new Error('Frozen parser tarball checksum mismatch');
      } else {
        const root=cleanSource(dir!);checkRepo(root,projects,checked);
        artifact=join(packDir,`${manifest.name.replace(/^@/,'').replaceAll('/','-')}-${manifest.version}.tgz`);
        run(['bun','pm','pack','--filename',artifact],dir,true);
        cleanSource(root);
      }
      candidate.manifest=inspectArtifact(artifact,manifest);candidate.artifact=artifact;
      run(['bun','publish',artifact,'--dry-run','--access','public','--registry',registryURL],base,true);
      ready.push(candidate);
    } catch(error) {blocked.push(entry.name);console.error(`BLOCKED ${entry.name}: ${(error as Error).message}`);}
  }
  // Validate every required packed dependency, including dependencies excluded by --only.
  const readyMap=new Map(ready.map(p=>[p.manifest.name,p.manifest]));
  const versionsCache=new Map<string,string[]>();
  const lookup=async(name:string)=>{
    if(!versionsCache.has(name))versionsCache.set(name,Object.keys((await metadata(name)).versions));
    return versionsCache.get(name)!;
  };
  const safe:Candidate[]=[];
  for(const p of ready) {
    try {await assertDependencies(p.manifest,readyMap,lookup);safe.push(p);}
    catch(error){readyMap.delete(p.manifest.name);blocked.push(p.manifest.name);console.error(`BLOCKED ${p.manifest.name}: ${(error as Error).message}`);}
  }
  if(publish)for(const p of safe) {
    const fresh=await metadata(p.manifest.name);
    if(Object.hasOwn(fresh.versions,p.manifest.version)){await checkPublished(fresh,p.manifest.version);console.log(`Already published: ${p.manifest.name}`);continue;}
    const tag=p.manifest.version.includes('-')?'next':'latest';
    run(['bun','publish',p.artifact!,'--access','public','--registry',registryURL,'--tag',tag,'--auth-type','web'],base,true);
    const actual=await metadata(p.manifest.name);await checkPublished(actual,p.manifest.version);
    console.log(`VERIFIED ${p.manifest.name}@${p.manifest.version}`);
  }
  console.log(`\n${publish?'Published/verified':'Prepared'} ${safe.length}; blocked ${blocked.length}. No source files were rewritten and no versions were bumped.`);
  if(blocked.length)process.exitCode=1;
}
if(import.meta.main)main().catch(error=>{console.error((error as Error).message);process.exitCode=1;});
