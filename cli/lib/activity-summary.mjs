// One line per MCP tool call for activity.jsonl and the Agent window's feed:
// what the agent looked at or changed, in a few words ("listed 15 scenes",
// "rendered hero-island at 2×"). Edits already carry a summary; read
// commands get one here (first), else the result's summary, else the first
// line of the command's human format.

const plural = (/** @type {number} */ n, /** @type {string} */ word, /** @type {string} */ many = `${word}s`) => `${n} ${n === 1 ? word : many}`;
const count = (/** @type {unknown} */ v) => (Array.isArray(v) ? v.length : 0);

/** @type {Record<string, (r: any, p: any) => string | undefined>} */
const READS = {
  'site.info': (r) => `site ${r.root}: ${plural(r.scenes, 'scene')}, ${plural(r.objects, 'object')}`,
  'palette.get': (r) => `listed ${plural(count(r.colors), 'palette color')}`,
  'scene.list': (r) => `listed ${plural(count(r.scenes), 'scene')}`,
  'scene.show': (r) => `showed scene ${r.name} (${plural(r.items, 'item')}${r.motion ? ', motion' : ''})`,
  'scene.items': (r) => `listed ${plural(count(r.items), 'item')} of ${r.scene}`,
  'scene.render': (r) => `rendered ${r.scene} at ${r.scale}×${r.time ? `, ${r.time} ms` : ''}`,
  'object.list': (r) => `listed ${plural(count(r.objects), 'object')}`,
  'object.show': (r) => `showed object ${r.name}${r.description ? ` (${r.description})` : ''}`,
  'object.render': (r) => `rendered ${r.object} at ${r.scale}×${r.frame ? `, frame ${r.frame}` : ''}${r.time ? `, ${r.time} ms` : ''}`,
  'object.pick': (r) => `picked ${r.object} at ${r.x},${r.y}: ${r.color ?? 'empty'}`,
  check: (r) =>
    `checked ${r.target?.name ?? 'every source'} (${plural(r.checked ?? 0, 'file')}): ${count(r.problems) ? plural(count(r.problems), 'problem') : 'no problems'}`,
  'proposal.list': (r) => `listed ${plural(count(r.proposals), 'proposal')}`,
  'proposal.show': (r) => `showed proposal ${r.id} (${r.doc}, ${plural(count(r.summary), 'edit')})`,
  'proposal.render': (r) => `rendered proposal ${r.id} (${r.doc}) at ${r.scale}×`,
  'activity.list': (r) => `listed ${plural(count(r.entries), 'activity entry', 'activity entries')}`,
  'doc.status': (r) => (r.docs ? `${plural(count(r.docs), 'doc')} open` : `${r.doc}: ${r.dirty ? 'unsaved changes' : 'saved'}`),
  'doc.open': (r) => `opened ${r.doc}`,
};

/**
 * The activity line for a command's result.
 * @param {{ name: string, format?: (r: any, p: any) => string }} cmd
 * @param {any} result
 * @param {any} params
 */
export function activitySummary(cmd, result, params) {
  try {
    const read = READS[cmd.name]?.(result ?? {}, params ?? {});
    if (read) return read;
  } catch {
    // fall through
  }
  if (typeof result?.summary === 'string') return result.summary;
  try {
    const text = cmd.format ? cmd.format(result, params) : '';
    return text.split('\n')[0].replace(/\s+/g, ' ').trim().slice(0, 200);
  } catch {
    return '';
  }
}
