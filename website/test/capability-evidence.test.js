import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { readyCapabilitiesMissingEvidence } from '../src/lib/capabilities.js';

const root = join(import.meta.dirname, '../..');

test('ready capabilities require a reproducible test ID and commit', () => {
  const header = '| Plataforma | Sistema | Núcleo/ruta observada | Arranque A/V/input | SRAM/estados/FF/rewind | Companion / datos live | Link | Evidencia | Próxima verificación |';
  const separator = '|---|---|---|---|---|---|---|---|---|';
  const valid = '| Windows | GB | mGBA | Ready | No probado | No probado | No probado | testId=gb.boot commit=abcdef1 | - |';
  const invalid = '| Android | GBA | mGBA | No probado | Listo | No probado | No probado | E07 | - |';
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

test('the checked-in capability matrix has no ready claim without test evidence', () => {
  const matrix = readFileSync(join(root, 'docs/vyntra-capabilities.md'), 'utf8');
  assert.deepEqual(readyCapabilitiesMissingEvidence(matrix), []);
});
