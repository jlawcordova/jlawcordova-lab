// The param checker every door shares. A command's params are a JSON Schema
// object; this checks a small, well-defined subset of it: type (string,
// integer, number, boolean, array, object), enum, pattern, minimum, maximum,
// minItems, maxItems, items, required, default and description. Unknown
// params are refused. With `coerce`, strings (from the command line) become
// the declared type: "3" → 3, "true"/"false" → booleans, "14,6,1" → [14, 6, 1],
// and a JSON string → an object.

import { UsageError } from './errors.mjs';

/**
 * @typedef {{
 *   type?: 'string' | 'integer' | 'number' | 'boolean' | 'array' | 'object',
 *   description?: string,
 *   enum?: unknown[],
 *   pattern?: string,
 *   minimum?: number,
 *   maximum?: number,
 *   minItems?: number,
 *   maxItems?: number,
 *   items?: Schema,
 *   properties?: Record<string, Schema>,
 *   required?: string[],
 *   default?: unknown,
 *   additionalProperties?: boolean,
 * }} Schema
 */

const TYPES = ['string', 'integer', 'number', 'boolean', 'array', 'object'];

/**
 * Checks (and with `coerce`, converts) params against a command's schema.
 * Returns a new object with defaults filled in. Throws one UsageError that
 * lists every problem.
 * @param {Schema} schema  an object schema
 * @param {Record<string, unknown> | undefined} input
 * @param {{ coerce?: boolean, command?: string }} [options]
 * @returns {Record<string, any>}
 */
export function checkParams(schema, input, { coerce = false, command } = {}) {
  /** @type {string[]} */
  const problems = [];
  const props = schema.properties ?? {};
  const given = input ?? {};
  if (typeof given !== 'object' || given === null || Array.isArray(given)) {
    throw new UsageError(`${command ?? 'params'}: params must be an object`);
  }
  /** @type {Record<string, any>} */
  const out = {};
  for (const key of Object.keys(given)) {
    if (!(key in props)) problems.push(`unknown param ${key}${suggest(key, Object.keys(props))}`);
  }
  for (const [key, prop] of Object.entries(props)) {
    let value = given[key];
    if (value === undefined) {
      if (schema.required?.includes(key)) problems.push(`${key} is required`);
      else if (prop.default !== undefined) out[key] = structuredClone(prop.default);
      continue;
    }
    if (coerce) value = coerceValue(prop, value);
    const before = problems.length;
    checkValue(prop, value, key, problems);
    if (problems.length === before) out[key] = value;
  }
  if (problems.length) {
    const prefix = command ? `${command}: ` : '';
    throw new UsageError(`${prefix}${problems.join('; ')}`, { details: problems });
  }
  return out;
}

/**
 * @param {Schema} schema
 * @param {unknown} value
 * @param {string} where
 * @param {string[]} problems
 */
function checkValue(schema, value, where, problems) {
  const { type } = schema;
  if (type && !TYPES.includes(type)) throw new Error(`schema for ${where}: unsupported type ${type}`);
  if (type === 'integer' && !Number.isInteger(value)) return problems.push(`${where} must be a whole number, got ${show(value)}`);
  if (type === 'number' && (typeof value !== 'number' || !Number.isFinite(value))) return problems.push(`${where} must be a number, got ${show(value)}`);
  if (type === 'string' && typeof value !== 'string') return problems.push(`${where} must be a string, got ${show(value)}`);
  if (type === 'boolean' && typeof value !== 'boolean') return problems.push(`${where} must be true or false, got ${show(value)}`);
  if (type === 'array' && !Array.isArray(value)) return problems.push(`${where} must be a list, got ${show(value)}`);
  if (type === 'object' && (typeof value !== 'object' || value === null || Array.isArray(value))) {
    return problems.push(`${where} must be an object, got ${show(value)}`);
  }
  if (schema.enum && !schema.enum.includes(value)) return problems.push(`${where} must be one of ${schema.enum.join(', ')}, got ${show(value)}`);
  if (typeof value === 'number') {
    if (schema.minimum !== undefined && value < schema.minimum) problems.push(`${where} must be at least ${schema.minimum}, got ${value}`);
    if (schema.maximum !== undefined && value > schema.maximum) problems.push(`${where} must be at most ${schema.maximum}, got ${value}`);
  }
  if (typeof value === 'string' && schema.pattern && !new RegExp(schema.pattern).test(value)) {
    problems.push(`${where} ${show(value)} doesn't match ${schema.pattern}`);
  }
  if (Array.isArray(value)) {
    if (schema.minItems !== undefined && value.length < schema.minItems) problems.push(`${where} needs at least ${schema.minItems} items, got ${value.length}`);
    if (schema.maxItems !== undefined && value.length > schema.maxItems) problems.push(`${where} takes at most ${schema.maxItems} items, got ${value.length}`);
    if (schema.items) value.forEach((v, i) => checkValue(/** @type {Schema} */ (schema.items), v, `${where}[${i}]`, problems));
  }
  if (type === 'object' && schema.properties) {
    const obj = /** @type {Record<string, unknown>} */ (value);
    for (const key of schema.required ?? []) if (obj[key] === undefined) problems.push(`${where}.${key} is required`);
    for (const [key, v] of Object.entries(obj)) {
      const prop = schema.properties[key];
      if (prop) checkValue(prop, v, `${where}.${key}`, problems);
      else if (schema.additionalProperties === false) problems.push(`${where}: unknown property ${key}`);
    }
  }
}

/**
 * Converts a command-line string (or list of strings) to the schema's type.
 * Anything it can't convert is returned as it is, for checkValue to report.
 * @param {Schema} schema
 * @param {unknown} value
 * @returns {unknown}
 */
export function coerceValue(schema, value) {
  switch (schema.type) {
    case 'integer':
    case 'number': {
      if (typeof value !== 'string') return value;
      const text = value.trim();
      const ok = schema.type === 'integer' ? /^[-+]?\d+$/.test(text) : /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(text);
      return ok ? Number(text) : value;
    }
    case 'boolean': {
      if (typeof value !== 'string') return value;
      const text = value.trim().toLowerCase();
      if (['true', 'yes', '1', 'on'].includes(text)) return true;
      if (['false', 'no', '0', 'off'].includes(text)) return false;
      return value;
    }
    case 'array': {
      const list = typeof value === 'string' ? [value] : value;
      if (!Array.isArray(list)) return value;
      const items = schema.items ?? {};
      // A list of lists ([[3, 4], [5, 6]]): "3,4 5,6" or "3,4;5,6", or the
      // flag (or positional) repeated, one inner list each.
      if (items.type === 'array') {
        return list.flatMap((v) => (typeof v === 'string' ? v.split(/[\s;]+/).filter(Boolean).map((group) => coerceValue(items, group)) : [coerceValue(items, v)]));
      }
      // "14,6,1" or repeated flags; strings split on commas.
      const flat = list.flatMap((v) => (typeof v === 'string' ? (v === '' ? [] : v.split(',').map((s) => s.trim())) : [v]));
      return flat.map((v) => coerceValue(items, v));
    }
    case 'object': {
      if (typeof value !== 'string') return value;
      try {
        return JSON.parse(value);
      } catch {
        return value;
      }
    }
    default:
      return value;
  }
}

const show = (/** @type {unknown} */ v) => (typeof v === 'string' ? JSON.stringify(v) : Array.isArray(v) ? `[${v.map(show).join(', ')}]` : String(v));

/**
 * " (did you mean x?)" for a close name, else "".
 * @param {string} word
 * @param {string[]} options
 */
export function suggest(word, options) {
  let best = '';
  let bestScore = Infinity;
  for (const option of options) {
    const d = distance(word, option);
    if (d < bestScore) [best, bestScore] = [option, d];
  }
  return best && bestScore <= Math.max(2, Math.floor(word.length / 3)) ? ` (did you mean ${best}?)` : '';
}

/** Levenshtein distance. */
function distance(/** @type {string} */ a, /** @type {string} */ b) {
  const row = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let prev = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const tmp = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = tmp;
    }
  }
  return row[b.length];
}
