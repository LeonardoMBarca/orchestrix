import test from 'node:test';
import assert from 'node:assert/strict';
import {containedSettingsPosition} from '../src/settings-window.ts';

test('a dragged Settings panel stays contained across native window shrink and expansion',()=>{
 const preferred=Object.freeze({left:299,top:84});
 assert.deepEqual(containedSettingsPosition(preferred,{width:940,height:720},{width:1536,height:864}),preferred);
 const compact=containedSettingsPosition(preferred,{width:852,height:550},{width:902,height:620});
 assert.deepEqual(compact,{left:50,top:70});assert.equal(compact.left+852,902);assert.equal(compact.top+550,620);
 assert.deepEqual(containedSettingsPosition(preferred,{width:940,height:720},{width:1536,height:864}),preferred,'temporary containment does not overwrite deliberate drag position');
});
test('drag bounds use the complete rendered panel rather than a visible header fragment',()=>{
 assert.deepEqual(containedSettingsPosition({left:1500,top:900},{width:940,height:720},{width:1536,height:864}),{left:596,top:144});
 assert.deepEqual(containedSettingsPosition({left:-200,top:-80},{width:852,height:550},{width:902,height:620}),{left:0,top:0});
 assert.deepEqual(containedSettingsPosition({left:100,top:40},{width:886,height:604},{width:902,height:620}),{left:16,top:16},'a manually enlarged panel uses its actual post-CSS viewport dimensions');
});
