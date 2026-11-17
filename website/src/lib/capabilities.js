const MATRIX_HEADER = '| Plataforma | Sistema | Núcleo/ruta observada |';
const STATUS_COLUMNS = new Map([
  [3, 'av'],
  [4, 'sram'],
  [5, 'companion'],
  [6, 'link'],
]);

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

function hasReproducibleEvidence(evidence, capability) {
  return evidence.split(';').some((record) => {
    const cell = new RegExp(`(?:^|\\s)cell=${capability}(?:\\s|$)`, 'i').test(record);
    const testId = /(?:^|\s)testId=[A-Za-z0-9_.:-]+(?:\s|$)/i.test(record);
    const build = /(?:^|\s)build=[A-Za-z0-9._+-]{1,64}(?:\s|$)/i.test(record);
    const commit = /(?:^|\s)commit=[a-f0-9]{7,40}(?:\s|$)/i.test(record);
    const coreHash = /(?:^|\s)coreHash=[a-f0-9]{64}(?:\s|$)/i.test(record);
    return cell && testId && build && commit && coreHash;
  });
}

export function readyCapabilitiesMissingEvidence(markdown) {
  const lines = markdown.split(/\r?\n/);
  const headerIndex = lines.findIndex((line) => line.startsWith(MATRIX_HEADER));
  if (headerIndex < 0) throw new Error('Capability matrix table not found');

  const headers = cells(lines[headerIndex]);
  const findings = [];
  for (let index = headerIndex + 2; index < lines.length && lines[index].startsWith('|'); index += 1) {
    const row = cells(lines[index]);
    if (row.length !== headers.length) {
      findings.push({
        line: index + 1,
        system: row[1] || '',
        capability: 'matrix row shape',
        status: 'invalid column count',
      });
      continue;
    }
    for (const [column, capabilityId] of STATUS_COLUMNS) {
      if (!isReady(row[column])) continue;
      if (hasReproducibleEvidence(row[7], capabilityId)) continue;
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
