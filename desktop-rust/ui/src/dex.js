const normalize = value => String(value ?? '').normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().trim();

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
