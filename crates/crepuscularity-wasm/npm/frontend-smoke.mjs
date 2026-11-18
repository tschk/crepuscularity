import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const fixtures = [
  ['App.crepus', 'if {true}\n  p\n    "ok"'],
  ['App.tsx', '<if condition={true}><p>ok</p></if>'],
  ['App.svelte', '{#if true}<p>ok</p>{/if}'],
  ['App.vue', '<template><p v-if="true">ok</p></template>'],
  ['App.astro', '{true && <p>ok</p>}'],
  ['App.component.html', '@if (true) { <p>ok</p> }'],
];
for (const path of ['./wasm/crepuscularity_wasm.js', '../pkg/crepuscularity_wasm.js']) {
  const parser = require(path);
  for (const [filename, source] of fixtures) {
    const ir = JSON.parse(parser.parse_template_json(source, filename));
    assert.equal(ir.version, parser.ir_version(), filename);
    assert.equal(ir.root[0].kind, 'if', filename);
    assert.match(JSON.stringify(ir), /"ok"/, filename);
  }
  console.log(`PASS ${path}: six executable parser frontends`);
}
