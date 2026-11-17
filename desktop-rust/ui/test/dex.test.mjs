import test from 'node:test';
import assert from 'node:assert/strict';
import {calculateTypeEffectiveness,filterDexEntries,loadSavedTypeChart,saveTypeChart} from '../src/dex.js';

const entries=[
 {number:1,name:'Bulbasaur',generation:1},
 {number:2,name:'Ivysaur',generation:1},
 {number:152,name:'Chikorita',generation:2},
 {number:252,name:'Treecko',generation:3},
 {number:906,name:'Sprigatito',generation:9},
];

test('Dex search matches names without case or accent sensitivity and narrows by generation',()=>{
 assert.deepEqual(filterDexEntries(entries,'CHIKO',2).map(entry=>entry.number),[152]);
 assert.deepEqual(filterDexEntries(entries,'treecko',2),[]);
 assert.deepEqual(filterDexEntries(entries,'sprig',9).map(entry=>entry.number),[906]);
 assert.deepEqual(filterDexEntries(entries,'bulba','all').map(entry=>entry.number),[1]);
});

test('numeric Dex lookup accepts a hash and leading zeroes as an exact number',()=>{
 assert.deepEqual(filterDexEntries(entries,'#001','all').map(entry=>entry.number),[1]);
 assert.deepEqual(filterDexEntries(entries,'152',1),[]);
 assert.deepEqual(filterDexEntries(entries,'386','all'),[]);
});

test('type effectiveness combines dual types and preserves immunity',()=>{
 const chart={default_percent:100,matchups:[
  {attack:1,defense:2,percent:200},
  {attack:1,defense:3,percent:50},
  {attack:1,defense:4,percent:0},
 ]};
 assert.equal(calculateTypeEffectiveness(chart,1,[2,3]),100);
 assert.equal(calculateTypeEffectiveness(chart,1,[2,4]),0);
 assert.equal(calculateTypeEffectiveness(chart,1,[2]),200);
});

test('manual type chart choice persists per ROM and rejects unknown profiles',()=>{
 const values=new Map();
 const storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value)};
 const charts=[{dataset_id:'pokemon-gen9-standard-type-chart'},{dataset_id:'pokemon-gen6-glazed-type-chart'}];
 assert.equal(loadSavedTypeChart('rom-a',charts,storage),null);
 saveTypeChart('rom-a','pokemon-gen9-standard-type-chart',storage);
 saveTypeChart('rom-b','pokemon-gen6-glazed-type-chart',storage);
 assert.equal(loadSavedTypeChart('rom-a',charts,storage),'pokemon-gen9-standard-type-chart');
 assert.equal(loadSavedTypeChart('rom-b',charts,storage),'pokemon-gen6-glazed-type-chart');
 saveTypeChart('rom-a','missing-chart',storage);
 assert.equal(loadSavedTypeChart('rom-a',charts,storage),null);
 assert.equal(loadSavedTypeChart('',charts,storage),null);
});
