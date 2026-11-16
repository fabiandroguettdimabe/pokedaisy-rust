import {test} from 'node:test';
import assert from 'node:assert/strict';
import {chromium} from 'playwright-core';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';

test('offline UI: navigation, filtering, configuration IPC and missing-core state',async()=>{
 const browser=await chromium.launch({channel:'msedge',headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1360,height:880}});
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(()=>{window.commands=[];window.ipc={postMessage:body=>window.commands.push(JSON.parse(body))};});
  await page.goto(pathToFileURL(resolve('dist/index.html')).href);
  await page.waitForFunction(()=>typeof window.vyntraState==='function');
  await page.evaluate(()=>window.vyntraState({
   config:{library:'C:/fixtures/roms',saves:'C:/fixtures/saves',states:'C:/fixtures/states',system:'C:/fixtures/bios',cores:[null,null,null],volume:80,smooth:true,fullscreen:false,recursive:true,fast_forward:2,autosave:20,reduced_motion:false,keys:['Z','X','F','Tab','Return','Backspace','Up','Down','Left','Right','Q','E','1','2']},
   games:[{id:0,title:'Synthetic GB fixture',system:'GB',path:'C:/fixtures/test.gb',missing:false,core:false},{id:1,title:'Synthetic GBA fixture',system:'GBA',path:'C:/fixtures/test.gba',missing:false,core:false}],
   cores:[{id:0,system:'GBC',path:null,installed:false},{id:1,system:'GBA',path:null,installed:false},{id:2,system:'DS',path:null,installed:false}],saves:[],scanning:false,
  }));
  assert.equal(await page.locator('.game-card').count(),2);
  assert.equal(await page.getByRole('button',{name:'Jugar',exact:true}).isDisabled(),true);
  await page.getByRole('textbox',{name:'Buscar juegos'}).fill('Synthetic GBA');
  assert.equal(await page.locator('.game-card').count(),1);
  await page.getByRole('button',{name:'Limpiar búsqueda'}).click();
  await page.locator('.system-row').filter({hasText:'GBA'}).click();
  assert.equal(await page.locator('.game-card').count(),1);
  await page.getByRole('button',{name:'Configuración',exact:true}).click();
  await page.getByRole('switch',{name:'Reducir animaciones'}).click();
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'set',field:'reduced_motion',value:true});
  await page.getByRole('button',{name:'Carpetas',exact:true}).click();
  await page.getByRole('button',{name:'Cambiar…',exact:true}).first().click();
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'choose',field:'library'});
  await page.getByRole('button',{name:'Controles',exact:true}).click();
  await page.getByRole('button',{name:'Z',exact:true}).click();
  await page.getByRole('button',{name:'Pulsa una tecla…',exact:true}).waitFor();
  await page.keyboard.press('c');
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'set',field:'key',index:0,value:'c'});
  await page.getByRole('button',{name:'Vyntra Vault',exact:true}).click();
  assert.equal(await page.getByText('No hay guardados en las carpetas configuradas.').isVisible(),true);
  await page.setViewportSize({width:960,height:640});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);
  assert.deepEqual(errors,[]);
 }finally{await browser.close();}
});
