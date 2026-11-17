import {test} from 'node:test';
import assert from 'node:assert/strict';
import {chromium} from 'playwright-core';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';

function expectVersionedCommand(page){
 const ids=new Set();
 return async(action,payload={},context={})=>{
  const command=await page.evaluate(()=>window.commands.at(-1));
  const commands=await page.evaluate(()=>window.commands);
  assert.equal(command.protocol_version,1);
  assert.match(command.command_id,/^ui-\d+-\d+$/);
  assert.equal(ids.has(command.command_id),false);
  assert.equal(new Set(commands.map(item=>item.command_id)).size,commands.length);
  ids.add(command.command_id);
  assert.equal(command.action,action);
  assert.deepEqual(command.payload,payload);
  for(const key of ['session_id','epoch','expected_revision'])assert.equal(command[key],context[key]);
  return command;
 };
}

test('offline UI: navigation, filtering, configuration IPC and missing-core state',async()=>{
 const browser=await chromium.launch({channel:'msedge',headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1360,height:880}});
  const expectCommand=expectVersionedCommand(page);
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(()=>{window.commands=[];window.ipc={postMessage:body=>window.commands.push(JSON.parse(body))};});
  await page.goto(pathToFileURL(resolve('dist/index.html')).href);
  await page.waitForFunction(()=>typeof window.vyntraState==='function');
  await page.evaluate(()=>window.vyntraState(window.testState={
   config:{library:'C:/fixtures/roms',saves:'C:/fixtures/saves',states:'C:/fixtures/states',system:'C:/fixtures/bios',cores:[null,null,null],volume:80,smooth:true,fullscreen:false,recursive:true,fast_forward:2,autosave:20,reduced_motion:false,keys:['Z','X','F','Tab','Return','Backspace','Up','Down','Left','Right','Q','E','1','2']},
   games:[{id:0,storage_id:'gb-fixture',title:'Synthetic GB fixture',system:'GB',path:'C:/fixtures/test.gb',missing:false,core:false,note:''},{id:1,storage_id:'gba-fixture',title:'Synthetic GBA fixture',system:'GBA',path:'C:/fixtures/test.gba',missing:false,core:false,note:''}],
   cores:[{id:0,system:'GBC',path:null,installed:false},{id:1,system:'GBA',path:'C:/cores/test.dll',installed:true},{id:2,system:'DS',path:null,installed:false}],saves:[],scanning:false,
   reference_type_chart:{schema_version:1,dataset_id:'pokemon-gen3-type-chart',generation:3,source_sha256:'a'.repeat(64),license:'GPL-3.0-only',default_percent:100,types:[{id:8,name:'Steel'},{id:10,name:'Fire'},{id:12,name:'Grass'}],matchups:[]},
  }));
  assert.equal(await page.locator('.game-card').count(),2);
  await page.getByRole('button',{name:'Importar carpetas',exact:true}).click();
  await expectCommand('import_roots');
  assert.equal(await page.getByRole('button',{name:'Jugar',exact:true}).isDisabled(),true);
  await page.getByRole('textbox',{name:'Nota del juego'}).fill('Usar partida del bosque');
  await page.getByRole('button',{name:'Guardar nota'}).click();
  await expectCommand('game_note',{storage_id:'gb-fixture',value:'Usar partida del bosque'});
  await page.getByRole('textbox',{name:'Buscar juegos'}).fill('Synthetic GBA');
  assert.equal(await page.locator('.game-card').count(),1);
  await page.getByRole('button',{name:'Limpiar búsqueda'}).click();
  await page.locator('.system-row').filter({hasText:'GBA'}).click();
  assert.equal(await page.locator('.game-card').count(),1);
  await page.getByRole('button',{name:'Configuración',exact:true}).click();
  await page.getByRole('switch',{name:'Reducir animaciones'}).click();
  await expectCommand('set',{field:'reduced_motion',value:true});
  await page.getByRole('button',{name:'Restablecer sección'}).click();
  await expectCommand('reset_settings_section',{section:'general'});
  await page.getByRole('button',{name:'Carpetas',exact:true}).click();
  await page.getByRole('button',{name:'Cambiar…',exact:true}).first().click();
  await expectCommand('choose',{field:'library'});
  await page.getByRole('button',{name:'Controles',exact:true}).click();
  await page.getByRole('button',{name:'Z',exact:true}).click();
  await page.getByRole('button',{name:'Pulsa una tecla…',exact:true}).waitFor();
  await page.keyboard.press('c');
  await expectCommand('set',{field:'key',index:0,value:'c'});
  await page.getByRole('button',{name:'Vyntra Vault',exact:true}).click();
  await page.getByRole('button',{name:'Importar paquete',exact:true}).click();
  await expectCommand('import_vault_bundle');
  await page.getByRole('button',{name:'Revisar migración antigua'}).click();
  await expectCommand('migration_preview');
  await page.evaluate(()=>window.vyntraAck({ok:true,result:{migration:{schema:1,items:[]}}}));
  assert.equal(await page.getByText('No se encontraron guardados heredados.').isVisible(),true);
  assert.equal(await page.getByText('No hay guardados en las carpetas configuradas.').isVisible(),true);
  await page.getByRole('button',{name:'Núcleos',exact:true}).last().click();
  await page.getByRole('button',{name:'Probar',exact:true}).nth(1).click();
  await expectCommand('probe_core',{index:1});
  await page.getByRole('button',{name:'Acompañante',exact:true}).click();
  await page.getByLabel('Ataque').selectOption('10');
  await page.getByLabel('Defensa 1').selectOption('12');
  await page.getByLabel('Defensa 2').selectOption('8');
  await page.getByRole('button',{name:'Calcular',exact:true}).click();
  await expectCommand('type_matchup',{attack:10,defenses:[12,8]});
  await page.evaluate(()=>window.vyntraAck({ok:true,result:{dataset_id:'pokemon-gen3-type-chart',attack_type:'Fire',defense_types:['Grass','Steel'],multiplier_percent:400}}));
  assert.equal(await page.getByText('4×').isVisible(),true);
  await page.evaluate(()=>window.vyntraImportProgress({phase:'scan',member:'Revisando carpetas',completed_bytes:256,total_bytes:100000}));
  assert.equal(await page.locator('.toast strong').getByText('Revisando carpetas',{exact:true}).isVisible(),true);
  await page.evaluate(()=>window.vyntraImportProgress({phase:'import',member:'Pokemon.gba',completed_bytes:512,total_bytes:1024,batch_item:2,batch_total:5}));
  assert.equal(await page.getByText('Pokemon.gba').isVisible(),true);
  assert.equal(await page.getByText('Importando ROM 2 de 5').isVisible(),true);
  await page.getByRole('button',{name:'Cancelar',exact:true}).click();
  await expectCommand('cancel_import');
  await page.evaluate(()=>window.vyntraImportDone({imported:0,duplicates:0,rejected:0,cancelled:true}));
  assert.equal(await page.getByText('Importación cancelada · Importadas: 0 · duplicadas: 0 · rechazadas: 0').isVisible(),true);
  await page.screenshot({path:resolve('../../.taste-skill/companion/companion-desktop.png'),fullPage:true});
  await page.setViewportSize({width:390,height:840});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);
  await page.screenshot({path:resolve('../../.taste-skill/companion/companion-mobile.png'),fullPage:true});
  await page.setViewportSize({width:960,height:640});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);
  await page.evaluate(()=>{window.testState={...window.testState,session:{session_id:'test-session',epoch:2},command_revision:7};window.vyntraState(window.testState);window.vyntraPlayerSnapshot({title:'Synthetic GBA fixture',system:'GBA',screen:'playing',paused:false});});
  await page.getByRole('button',{name:'Pausar',exact:true}).click();
  await expectCommand('pause_player',{value:true},{session_id:'test-session',epoch:2,expected_revision:7});
  await page.getByRole('button',{name:'Guardar estado',exact:true}).click();
  await expectCommand('save_player_state',{}, {session_id:'test-session',epoch:2,expected_revision:7});
  await page.evaluate(()=>window.vyntraAck({ok:true,result:{player_command:{started:true,id:1,command:'save_player_state'}}}));
  assert.equal(await page.getByText('Player está procesando el estado…').isVisible(),true);
  await page.evaluate(()=>window.vyntraAck({ok:true,result:{player_command:{id:1,command:'save_state',ok:false,error:'Disco sin espacio'}}}));
  assert.equal(await page.getByText('Falló la operación de Player: Disco sin espacio').isVisible(),true);
  await page.getByRole('button',{name:'Cargar estado',exact:true}).click();
  await expectCommand('load_player_state',{}, {session_id:'test-session',epoch:2,expected_revision:7});
  await page.evaluate(()=>window.vyntraAck({ok:false,revision:8,error:{code:'stale_revision',reason:'La configuración cambió en otra ventana.'}}));
  assert.equal(await page.getByText('La configuración cambió en otra ventana. Se actualizó el estado; revisa los valores y vuelve a aplicar.').isVisible(),true);
  assert.deepEqual(errors,[]);
 }finally{await browser.close();}
});
