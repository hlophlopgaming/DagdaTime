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

test('fingerprint detects edits to existing tags and tag ranges',()=>{
  const tag={id:1,name:'Old',color:'#111111',parent_id:null};
  const range={id:2,tag_id:1,start_ts:100,end_ts:200,note:'Old'};
  const original=fingerprint([tag],[range]);
  for(const changed of [{...tag,name:'New'},{...tag,color:'#222222'},{...tag,parent_id:3}]){
    assert.notEqual(original,fingerprint([changed],[range]));
  }
  for(const changed of [{...range,start_ts:120},{...range,tag_id:3},{...range,note:'New'}]){
    assert.notEqual(original,fingerprint([tag],[changed]));
  }
});
