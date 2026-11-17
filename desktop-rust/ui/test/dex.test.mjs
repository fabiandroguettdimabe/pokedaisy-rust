import test from 'node:test';
import assert from 'node:assert/strict';
import {filterDexEntries} from '../src/dex.js';

const entries=[
 {number:1,name:'Bulbasaur',generation:1},
 {number:2,name:'Ivysaur',generation:1},
 {number:152,name:'Chikorita',generation:2},
 {number:252,name:'Treecko',generation:3},
];

test('Dex search matches names without case or accent sensitivity and narrows by generation',()=>{
 assert.deepEqual(filterDexEntries(entries,'CHIKO',2).map(entry=>entry.number),[152]);
 assert.deepEqual(filterDexEntries(entries,'treecko',2),[]);
 assert.deepEqual(filterDexEntries(entries,'bulba','all').map(entry=>entry.number),[1]);
});

test('numeric Dex lookup accepts a hash and leading zeroes as an exact number',()=>{
 assert.deepEqual(filterDexEntries(entries,'#001','all').map(entry=>entry.number),[1]);
 assert.deepEqual(filterDexEntries(entries,'152',1),[]);
 assert.deepEqual(filterDexEntries(entries,'386','all'),[]);
});
