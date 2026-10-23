import test from 'node:test';
import assert from 'node:assert/strict';
import {createConfirmation} from '../src/confirmation.ts';

function deferred(){let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};}

test('an asynchronous confirmation authorizes nothing until explicit acceptance',async()=>{
 const answer=deferred(),pending=[],errors=[],commands=[];
 const confirm=createConfirmation(()=>answer.promise,value=>pending.push(value),error=>errors.push(error));
 const action=(async()=>{if(await confirm('Authorize review'))commands.push('request-review');})();
 assert.deepEqual(commands,[]);
 assert.deepEqual(pending,[true]);
 answer.resolve(true);await action;
 assert.deepEqual(commands,['request-review']);
 assert.deepEqual(pending,[true,false]);
 assert.deepEqual(errors,[]);
});

test('Cancel returns without authorizing a mutation',async()=>{
 const answer=deferred(),pending=[],commands=[];
 const confirm=createConfirmation(()=>answer.promise,value=>pending.push(value),()=>assert.fail('Cancel is not an error'));
 const action=(async()=>{if(await confirm('Authorize correction'))commands.push('authorize-correction');})();
 answer.resolve(false);await action;
 assert.deepEqual(commands,[]);assert.deepEqual(pending,[true,false]);
});

test('a rejected native dialog fails closed, reports its error and permits a later confirmation',async()=>{
 const answer=deferred(),pending=[],errors=[],commands=[];let first=true;
 const confirm=createConfirmation(()=>{if(first){first=false;return answer.promise;}return true;},value=>pending.push(value),error=>errors.push(error));
 const action=(async()=>{if(await confirm('Authorize recovery'))commands.push('intervene');})();
 const denied=new Error('dialog permission denied');answer.reject(denied);await action;
 assert.deepEqual(commands,[]);assert.deepEqual(errors,[denied]);assert.deepEqual(pending,[true,false]);
 assert.equal(await confirm('Try again after the error'),true);assert.deepEqual(pending,[true,false,true,false]);
});

test('overlapping confirmation requests share a single consent opportunity',async()=>{
 const answer=deferred(),pending=[];let opened=0;
 const confirm=createConfirmation(()=>{opened++;return answer.promise;},value=>pending.push(value),()=>assert.fail('unexpected error'));
 const first=confirm('First request');assert.equal(await confirm('Duplicate request'),false);
 assert.equal(opened,1);assert.deepEqual(pending,[true]);
 answer.resolve(true);assert.equal(await first,true);assert.deepEqual(pending,[true,false]);
});

test('synchronous browser confirmation retains exact boolean semantics',async()=>{
 for(const value of [true,false,undefined,null,'yes',{}]){
  const pending=[],confirm=createConfirmation(()=>value,state=>pending.push(state),()=>assert.fail('unexpected error'));
  assert.equal(await confirm('Consent'),value===true);assert.deepEqual(pending,[true,false]);
 }
});
