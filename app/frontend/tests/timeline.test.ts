import {test} from 'node:test';
import assert from 'node:assert/strict';
import {clip,unionDuration,summarize,dayBounds,pixelRange,appColor,tracks,fixture,zoomView,continuousSpan} from '../src/timeline.ts';

test('half-open clipping and overlapping intervals do not double count',()=>{
  assert.equal(clip({start:0,end:10},{start:10,end:20}),null);
  assert.deepEqual(clip({start:0,end:15},{start:10,end:20}),{start:10,end:15});
  assert.equal(unionDuration([{start:0,end:15},{start:5,end:20},{start:20,end:25}],{start:10,end:30}),15);
  const rows=summarize([{start:0,end:15,key:'editor',label:'Editor',color:'red',detail:''},{start:5,end:20,key:'editor',label:'Editor',color:'red',detail:''}],{start:10,end:30});
  assert.equal(rows[0].duration,10);
  assert.equal(rows[0].share,50);
});

test('reverse drag, clipping and zoom stay within the day',()=>{
  assert.deepEqual(pixelRange({start:100,end:200},100,90,10),{start:110,end:190});
  assert.deepEqual(pixelRange({start:100,end:200},100,-20,120),{start:100,end:200});
  assert.deepEqual(zoomView({start:0,end:86_400_000},{start:0,end:86_400_000},.5),{start:21_600_000,end:64_800_000});
});

test('local day boundaries account for DST instead of assuming 24 hours',()=>{
  const previous=process.env.TZ;process.env.TZ='Europe/Berlin';
  try{
    const spring=dayBounds('2026-03-29');assert.equal(spring.end-spring.start,23*3_600_000);
    const fall=dayBounds('2026-10-25');assert.equal(fall.end-fall.start,25*3_600_000);
  }finally{if(previous===undefined)delete process.env.TZ;else process.env.TZ=previous;}
});

test('5000-event fixture produces four tracks and stable app colors',()=>{
  const data=fixture('2026-10-13',5000);
  assert.equal(data.events.filter(event=>event.bucket==='window').length,5000);
  const result=tracks(data,dayBounds('2026-10-13'));
  assert.deepEqual(result.map(track=>track.name),['Tags','Computer','Applications','Documents']);
  assert.equal(result[2].segments.length,5000);
  assert.equal(appColor('Editor'),appColor('Editor'));
  assert.notEqual(appColor('Editor'),appColor('Browser'));
  const whole=summarize(result[2].segments,dayBounds('2026-10-13'));
  const selected=summarize(result[2].segments,{start:result[2].segments[0].start,end:result[2].segments[0].end});
  assert.equal(selected.length,1);
  assert.equal(selected[0].share,100);
  assert.ok(whole.reduce((sum,row)=>sum+row.duration,0)>selected[0].duration);
});

test('double click selects the entire adjacent app span',()=>{
  const values=[{start:0,end:10},{start:10,end:20},{start:20,end:30}].map(value=>({...value,key:'editor',label:'Editor',color:'blue',detail:''}));
  assert.deepEqual(continuousSpan(values[2],values),{start:0,end:30});
});
