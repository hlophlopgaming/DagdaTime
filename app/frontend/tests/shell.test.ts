import {test} from 'node:test';
import assert from 'node:assert/strict';
import {statusView} from '../src/shell.ts';

test('tracking, paused and error have distinct shared status labels',()=>{
  assert.equal(statusView({state:'tracking',backends:[]}).label,'Tracking');
  assert.equal(statusView({state:'paused',backends:[]}).label,'Paused');
  const error = statusView({state:'error',backends:[{name:'window',state:'unsupported',error:'Enable the GNOME extension'}]});
  assert.equal(error.label,'Needs attention');
  assert.match(error.detail,/GNOME/);
  assert.equal(statusView(null).label,'Service offline');
});
