import { describe, expect, test } from 'bun:test';
import { ordered, status, validatePacked, assertDependencies, waitForAvailability } from './publish';
const registry = {name:'@test/a',versions:{'1.0.0':{name:'@test/a',version:'1.0.0'}},'dist-tags':{latest:'1.0.0'}};
const pkg=(name:string,dependencies:Record<string,string>={})=>({entry:{name,observedLatest:'1.0.0'},manifest:{name,version:'1.1.0',dependencies},registry});
describe('release decisions',()=>{
 test('skips existing and older versions; reports missing local source',()=>{
  expect(status('1.0.0',registry)).toBe('CURRENT');
  expect(status('0.9.0',registry)).toBe('OLDER LOCAL');
  expect(status('1.1.0',registry)).toBe('PENDING');
  expect(status(undefined,registry)).toBe('NO LOCAL SOURCE');
 });
 test('orders consumers after required dependencies',()=>{
  const consumer=pkg('@test/b',{'@test/a':'^1.1.0'}),dependency=pkg('@test/a');
  expect(ordered([consumer,dependency]).map(p=>p.manifest.name)).toEqual(['@test/a','@test/b']);
 });
 test('rejects incompatible selected dependency versions and cycles',()=>{
  expect(()=>ordered([pkg('@test/b',{'@test/a':'^2.0.0'}),pkg('@test/a')])).toThrow('does not accept');
  expect(()=>ordered([pkg('@test/b',{'@test/a':'^1.1.0'}),pkg('@test/a',{'@test/b':'^1.1.0'})])).toThrow('cycle');
 });
 test('rejects private packages and unresolved workspace/local runtime edges',()=>{
  expect(()=>validatePacked({name:'a',version:'1.0.0',private:true})).toThrow('private');
  for(const range of ['workspace:*','file:../x','link:../x','catalog:default','../x','/tmp/x']){
   expect(()=>validatePacked({name:'a',version:'1.0.0',dependencies:{x:range}})).toThrow('local dependency');
  }
  expect(()=>validatePacked({name:'a',version:'1.0.0',dependencies:{x:'^1.0.0'}})).not.toThrow();
 });
});

test('filtered consumer cannot publish with unavailable unselected runtime dependency',async()=>{
 const consumer={name:'@test/consumer',version:'1.1.0',dependencies:{'@test/core':'^1.1.0'}};
 await expect(assertDependencies(consumer,new Map(),async()=>['1.0.0'])).rejects.toThrow('neither available');
 await expect(assertDependencies(consumer,new Map(),async()=>['1.1.0'])).resolves.toBeUndefined();
 await expect(assertDependencies(consumer,new Map([['@test/core',{name:'@test/core',version:'1.1.0'}]]),async()=>[])).resolves.toBeUndefined();
});
test('required peer dependencies need a published or prepared version; optional peers do not',async()=>{
 const consumer={name:'consumer',version:'1.1.0',peerDependencies:{core:'^1.1.0',optional:'^5.0.0'},peerDependenciesMeta:{optional:{optional:true}}};
 await expect(assertDependencies(consumer,new Map(),async()=>['1.0.0'])).rejects.toThrow('required core');
 const calls:string[]=[];
 await assertDependencies(consumer,new Map(),async name=>{calls.push(name);return ['1.1.0'];});
 expect(calls).toEqual(['core']);
});

test('waits for npm processing without requiring a second upload',async()=>{
 let probes=0,sleeps=0;
 await waitForAvailability(async()=>++probes===3,4,0,async()=>{sleeps++;});
 expect(probes).toBe(3);expect(sleeps).toBe(2);
});
test('bounded availability timeout preserves accepted-upload guidance',async()=>{
 await expect(waitForAvailability(async()=>false,2,0,async()=>{})).rejects.toThrow('do not bump or republish');
});
test('permanent availability errors stop immediately',async()=>{
 let probes=0;
 await expect(waitForAvailability(async()=>{probes++;throw new Error('unexpected artifact origin');},4,0,async()=>{})).rejects.toThrow('origin');
 expect(probes).toBe(1);
});
