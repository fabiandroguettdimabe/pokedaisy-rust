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
