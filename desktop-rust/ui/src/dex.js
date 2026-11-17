const normalize = value => String(value ?? '').normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().trim();

const typeChartStorageKey = storageId => `vyntra.type-chart.v1:${storageId}`;

const gen3GameCodes = new Set(['AXV', 'AXP', 'BPE', 'BPR', 'BPG']);
const gen4GameCodes = new Set(['ADA', 'APA', 'CPU', 'IPK', 'IPG']);
const gen5GameCodes = new Set(['IRA', 'IRB', 'IRD', 'IRE']);

export function suggestTypeChartFromGameCode(system, rawCode) {
  const code = String(rawCode ?? '').toUpperCase();
  if (!/^[A-Z0-9]{4}$/.test(code)) return null;
  const prefix = code.slice(0, 3);
  if (system === 'GBA' && gen3GameCodes.has(prefix)) return 'pokemon-gen3-type-chart';
  if ((system === 'DS' || system === 'NDS') && gen4GameCodes.has(prefix)) return 'pokemon-gen4-standard-type-chart';
  if ((system === 'DS' || system === 'NDS') && gen5GameCodes.has(prefix)) return 'pokemon-gen5-standard-type-chart';
  return null;
}

export function inferTypeChartFromIdentity(identity) {
  if (identity?.confidence !== 'strict') return null;
  return suggestTypeChartFromGameCode(identity.system, identity.game_code);
}

export function loadSavedTypeChart(storageId, charts, storage = globalThis.localStorage) {
  if (!storageId || !storage) return null;
  try {
    const datasetId = storage.getItem(typeChartStorageKey(storageId));
    return charts.some(chart => chart.dataset_id === datasetId) ? datasetId : null;
  } catch {
    return null;
  }
}

export function saveTypeChart(storageId, datasetId, storage = globalThis.localStorage) {
  if (!storageId || !storage) return;
  try {
    storage.setItem(typeChartStorageKey(storageId), datasetId);
  } catch {
    // The chart remains usable for this session when local storage is unavailable.
  }
}

export function filterDexEntries(entries, query = '', generation = 'all') {
  const term = normalize(query);
  const numberQuery = term.match(/^#?(\d+)$/);
  const numericNumber = numberQuery ? Number(numberQuery[1]) : null;
  return entries.filter(entry => {
    if (generation !== 'all' && entry.generation !== Number(generation)) return false;
    if (!term) return true;
    if (numericNumber !== null) return entry.number === numericNumber;
    return normalize(entry.name).includes(term);
  });
}

export function calculateTypeEffectiveness(chart, attack, defenses) {
  let percent = chart.default_percent ?? 100;
  for (const defense of [...new Set(defenses)]) {
    const matchup = chart.matchups.find(pair => pair.attack === Number(attack) && pair.defense === Number(defense));
    percent = percent * (matchup?.percent ?? chart.default_percent ?? 100) / 100;
    if (percent === 0) break;
  }
  return Math.round(percent);
}
