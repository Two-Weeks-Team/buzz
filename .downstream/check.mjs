import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
const m=JSON.parse(readFileSync(new URL('./manifest.json',import.meta.url),'utf8'));
assert.equal(m.schema_version,1);
assert.equal(m.upstream,'https://github.com/block/buzz.git');
assert.equal(m.fork,'https://github.com/Two-Weeks-Team/buzz.git');
assert.equal(m.upstream_branch,'main');
assert.equal(m.integration_branch,'agentbase/main');
assert.match(m.upstream_base,/^[0-9a-f]{40}$/);
assert.equal(m.automatic_publish,false);
assert.ok(Array.isArray(m.patches));
const ids=new Set();
for(const p of m.patches){
  assert.ok(typeof p.id==='string' && !ids.has(p.id));ids.add(p.id);
  assert.ok(typeof p.summary==='string' && p.summary.length>0);
  assert.ok(['candidate','qualified'].includes(p.status));
  for(const field of ['paths','tests'])assert.ok(Array.isArray(p[field]) && p[field].length>0 && p[field].every(x=>typeof x==='string' && x.length>0));
  assert.ok(p.upstream_reference===null || /^https:\/\/github\.com\/block\/buzz\//.test(p.upstream_reference));
}
execFileSync('git',['merge-base','--is-ancestor',m.upstream_base,'HEAD'],{stdio:'pipe'});
console.log('Downstream metadata valid; this does not qualify runtime behavior or a release.');
