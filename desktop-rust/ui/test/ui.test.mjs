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
   games:[{id:0,storage_id:'gb-fixture',title:'Synthetic GB fixture',system:'GB',path:'C:/fixtures/test.gb',missing:false,core:false,note:''},{id:1,storage_id:'gba-fixture',title:'Synthetic GBA fixture',system:'GBA',path:'C:/fixtures/test.gba',missing:false,core:false,note:''}],
   cores:[{id:0,system:'GBC',path:null,installed:false},{id:1,system:'GBA',path:'C:/cores/test.dll',installed:true},{id:2,system:'DS',path:null,installed:false}],saves:[],scanning:false,
   reference_type_chart:{schema_version:1,dataset_id:'pokemon-gen3-type-chart',generation:3,source_sha256:'a'.repeat(64),license:'GPL-3.0-only',default_percent:100,types:[{id:8,name:'Steel'},{id:10,name:'Fire'},{id:12,name:'Grass'}],matchups:[]},
  }));
  assert.equal(await page.locator('.game-card').count(),2);
  await page.getByRole('button',{name:'Importar carpetas',exact:true}).click();
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'import_roots'});
  assert.equal(await page.getByRole('button',{name:'Jugar',exact:true}).isDisabled(),true);
  await page.getByRole('textbox',{name:'Nota del juego'}).fill('Usar partida del bosque');
  await page.getByRole('button',{name:'Guardar nota'}).click();
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'game_note',storage_id:'gb-fixture',value:'Usar partida del bosque'});
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
  await page.getByRole('button',{name:'Revisar migración antigua'}).click();
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'migration_preview'});
  await page.evaluate(()=>window.vyntraAck({ok:true,result:{migration:{schema:1,items:[]}}}));
  assert.equal(await page.getByText('No se encontraron guardados heredados.').isVisible(),true);
  assert.equal(await page.getByText('No hay guardados en las carpetas configuradas.').isVisible(),true);
  await page.getByRole('button',{name:'Núcleos',exact:true}).last().click();
  await page.getByRole('button',{name:'Probar',exact:true}).nth(1).click();
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'probe_core',index:1});
  await page.getByRole('button',{name:'Acompañante',exact:true}).click();
  await page.getByLabel('Ataque').selectOption('10');
  await page.getByLabel('Defensa 1').selectOption('12');
  await page.getByLabel('Defensa 2').selectOption('8');
  await page.getByRole('button',{name:'Calcular',exact:true}).click();
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'type_matchup',attack:10,defenses:[12,8]});
  await page.evaluate(()=>window.vyntraAck({ok:true,result:{dataset_id:'pokemon-gen3-type-chart',attack_type:'Fire',defense_types:['Grass','Steel'],multiplier_percent:400}}));
  assert.equal(await page.getByText('4×').isVisible(),true);
  await page.evaluate(()=>window.vyntraImportProgress({member:'Pokemon.gba',completed_bytes:512,total_bytes:1024,batch_item:2,batch_total:5}));
  assert.equal(await page.getByText('Pokemon.gba').isVisible(),true);
  assert.equal(await page.getByText('Importando ROM 2 de 5').isVisible(),true);
  await page.getByRole('button',{name:'Cancelar',exact:true}).click();
  assert.deepEqual(await page.evaluate(()=>window.commands.at(-1)),{action:'cancel_import'});
  await page.evaluate(()=>window.vyntraImportDone({imported:0,duplicates:0,rejected:0,cancelled:true}));
  assert.equal(await page.getByText('Importación cancelada · Importadas: 0 · duplicadas: 0 · rechazadas: 0').isVisible(),true);
  await page.screenshot({path:resolve('../../.taste-skill/companion/companion-desktop.png'),fullPage:true});
  await page.setViewportSize({width:390,height:840});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);
  await page.screenshot({path:resolve('../../.taste-skill/companion/companion-mobile.png'),fullPage:true});
  await page.setViewportSize({width:960,height:640});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);
  assert.deepEqual(errors,[]);
 }finally{await browser.close();}
});
