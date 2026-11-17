const MATRIX_HEADER = '| Plataforma | Sistema | Núcleo/ruta observada |';
const STATUS_COLUMNS = [3, 4, 5, 6];

function cells(line) {
  return line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map((cell) => cell.trim());
}

function isReady(status) {
  return /^\[?(?:ready|listo|lista|disponible)\b/i.test(status);
}

function hasReproducibleEvidence(evidence) {
  const testId = /(?:^|\s)testId=[A-Za-z0-9_.:-]+(?:\s|$)/i.test(evidence);
  const commit = /(?:^|\s)commit=[a-f0-9]{7,40}(?:\s|$)/i.test(evidence);
  return testId && commit;
}

export function readyCapabilitiesMissingEvidence(markdown) {
  const lines = markdown.split(/\r?\n/);
  const headerIndex = lines.findIndex((line) => line.startsWith(MATRIX_HEADER));
  if (headerIndex < 0) throw new Error('Capability matrix table not found');

  const headers = cells(lines[headerIndex]);
  const findings = [];
  for (let index = headerIndex + 2; index < lines.length && lines[index].startsWith('|'); index += 1) {
    const row = cells(lines[index]);
    if (row.length !== headers.length) continue;
    for (const column of STATUS_COLUMNS) {
      if (!isReady(row[column])) continue;
      if (hasReproducibleEvidence(row[7])) continue;
      findings.push({
        line: index + 1,
        system: row[1],
        capability: headers[column],
        status: row[column],
      });
    }
  }
  return findings;
}
