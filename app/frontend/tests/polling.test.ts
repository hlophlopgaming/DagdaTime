import {test} from 'node:test';
import assert from 'node:assert/strict';
import {fingerprint,setHidden,isHidden} from '../src/polling.ts';

test('fingerprint changes when polled data grows and stays stable otherwise',()=>{
  const events=[{id:1,end_ts:1_760_000_000_000},{id:2,end_ts:1_760_000_005_000}];
  const same=events.map(event=>({...event}));
  assert.equal(fingerprint(events,[]),fingerprint(same,[]));
  assert.notEqual(fingerprint(events,[]),fingerprint([events[0],{id:2,end_ts:1_760_000_006_000}],[]));
  assert.notEqual(fingerprint(events,[]),fingerprint([...events,{id:3,end_ts:1_760_000_007_000}],[]));
  assert.notEqual(fingerprint(events,[{id:1}]),fingerprint(events,[]));
});

test('visibility state can be toggled by the desktop shell',()=>{
  setHidden(true);assert.equal(isHidden(),true);
  setHidden(false);assert.equal(isHidden(),false);
});
