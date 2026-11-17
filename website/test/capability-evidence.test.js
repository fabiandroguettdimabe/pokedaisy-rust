import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { readyCapabilitiesMissingEvidence } from '../src/lib/capabilities.js';

const root = join(import.meta.dirname, '../..');

test('ready capabilities require complete reproducible evidence', () => {
  const header = '| Plataforma | Sistema | Núcleo/ruta observada | Arranque A/V/input | SRAM/estados/FF/rewind | Companion / datos live | Link | Evidencia | Próxima verificación |';
  const separator = '|---|---|---|---|---|---|---|---|---|';
  const valid = `| Windows | GB | mGBA | Ready | No probado | No probado | No probado | cell=av testId=gb.boot build=v1.2.3 commit=abcdef1 coreHash=${'a'.repeat(64)} | - |`;
  const invalid = '| Android | GBA | mGBA | No probado | Listo | No probado | No probado | cell=av testId=gb.boot commit=abcdef1 | - |';
  const findings = readyCapabilitiesMissingEvidence([header, separator, valid, invalid].join('\n'));

  assert.deepEqual(findings, [{ line: 4, system: 'GBA', capability: 'SRAM/estados/FF/rewind', status: 'Listo' }]);
});

test('a malformed capability row cannot hide a ready status', () => {
  const header = '| Plataforma | Sistema | Núcleo/ruta observada | Arranque A/V/input | SRAM/estados/FF/rewind | Companion / datos live | Link | Evidencia | Próxima verificación |';
  const separator = '|---|---|---|---|---|---|---|---|---|';
  const malformed = '| Windows | GBA | mGBA | Ready | No probado | No probado | No probado | testId=gba.boot commit=abcdef1 |';

  assert.deepEqual(readyCapabilitiesMissingEvidence([header, separator, malformed].join('\n')), [
    { line: 3, system: 'GBA', capability: 'matrix row shape', status: 'invalid column count' },
  ]);
});

test('each capability column accepts evidence scoped to its own cell', () => {
  const header = '| Plataforma | Sistema | Núcleo/ruta observada | Arranque A/V/input | SRAM/estados/FF/rewind | Companion / datos live | Link | Evidencia | Próxima verificación |';
  const separator = '|---|---|---|---|---|---|---|---|---|';
  const cases = [
    ['av', 3],
    ['sram', 4],
    ['companion', 5],
    ['link', 6],
  ];

  for (const [cell, column] of cases) {
    const row = ['Windows', 'GB', 'mGBA', 'No probado', 'No probado', 'No probado', 'No probado', `cell=${cell} testId=${cell}.test build=v1.2.3 commit=abcdef1 coreHash=${'a'.repeat(64)}`, '-'];
    row[column] = 'Ready';
    assert.deepEqual(readyCapabilitiesMissingEvidence([header, separator, `| ${row.join(' | ')} |`].join('\n')), [], cell);
  }
});

test('ready capabilities require a build and exact core hash', () => {
  const header = '| Plataforma | Sistema | Núcleo/ruta observada | Arranque A/V/input | SRAM/estados/FF/rewind | Companion / datos live | Link | Evidencia | Próxima verificación |';
  const separator = '|---|---|---|---|---|---|---|---|---|';
  const missingBuild = '| Windows | GB | mGBA | Ready | No probado | No probado | No probado | cell=av testId=gb.boot commit=abcdef1 coreHash=a'.concat('a'.repeat(63), ' | - |');
  const missingCoreHash = '| Windows | GB | mGBA | Ready | No probado | No probado | No probado | cell=av testId=gb.boot build=v1.2.3 commit=abcdef1 | - |';

  assert.equal(readyCapabilitiesMissingEvidence([header, separator, missingBuild].join('\n')).length, 1);
  assert.equal(readyCapabilitiesMissingEvidence([header, separator, missingCoreHash].join('\n')).length, 1);
});

test('required evidence fields cannot be split across semicolon records', () => {
  const header = '| Plataforma | Sistema | Núcleo/ruta observada | Arranque A/V/input | SRAM/estados/FF/rewind | Companion / datos live | Link | Evidencia | Próxima verificación |';
  const separator = '|---|---|---|---|---|---|---|---|---|';
  const splitEvidence = `| Windows | GB | mGBA | Ready | No probado | No probado | No probado | cell=av testId=gb.boot build=v1.2.3 commit=abcdef1; coreHash=${'a'.repeat(64)} | - |`;

  assert.equal(readyCapabilitiesMissingEvidence([header, separator, splitEvidence].join('\n')).length, 1);
});

test('the checked-in capability matrix has no ready claim without test evidence', () => {
  const matrix = readFileSync(join(root, 'docs/vyntra-capabilities.md'), 'utf8');
  assert.deepEqual(readyCapabilitiesMissingEvidence(matrix), []);
});
