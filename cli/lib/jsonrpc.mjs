// Newline-delimited JSON-RPC 2.0 over a pair of streams, for both servers:
// one JSON message per line in, one per line out. Nothing else is ever
// written to the output stream; logs go to stderr.

import { createInterface } from 'node:readline';

import { CommandError, UsageError } from './errors.mjs';

export const PARSE_ERROR = -32700;
export const INVALID_REQUEST = -32600;
export const METHOD_NOT_FOUND = -32601;
export const INVALID_PARAMS = -32602;
export const INTERNAL_ERROR = -32603;
/** A command ran and failed (data.code says how). */
export const COMMAND_ERROR = -32000;

export class RpcError extends Error {
  /** @param {number} code @param {string} message @param {unknown} [data] */
  constructor(code, message, data) {
    super(message);
    this.code = code;
    this.data = data;
  }
}

/**
 * An error as a JSON-RPC error object: usage errors are invalid params,
 * other command errors -32000, both with data: { code, details? }.
 * @param {unknown} err
 */
export function toRpcError(err) {
  if (err instanceof RpcError) return { code: err.code, message: err.message, ...(err.data !== undefined ? { data: err.data } : {}) };
  if (err instanceof CommandError) {
    const code = err instanceof UsageError ? INVALID_PARAMS : COMMAND_ERROR;
    return { code, message: err.message, data: { code: err.code, ...(err.details !== undefined ? { details: err.details } : {}) } };
  }
  const e = /** @type {Error} */ (err);
  return { code: INTERNAL_ERROR, message: e?.message ?? String(err), data: { code: 'internal' } };
}

/**
 * Serves JSON-RPC on input/output. `handle(method, params, message)` answers
 * requests (its value is the result; a throw is the error) and notifications
 * (its value is ignored). Requests run one at a time, in order.
 * @param {{
 *   input: NodeJS.ReadableStream,
 *   output: NodeJS.WritableStream,
 *   handle: (method: string, params: any, message: any) => Promise<any>,
 *   log?: (text: string) => void,
 * }} options
 */
export function serveLines({ input, output, handle, log = () => {} }) {
  let chain = Promise.resolve();
  let closed = false;
  const send = (/** @type {any} */ message) => {
    if (closed) return;
    output.write(`${JSON.stringify(message)}\n`);
  };
  const lines = createInterface({ input, crlfDelay: Infinity });
  /** @type {() => void} */
  let resolveDone = () => {};
  const done = new Promise((/** @type {(v?: unknown) => void} */ r) => (resolveDone = r));

  const one = async (/** @type {string} */ line) => {
    if (!line.trim()) return;
    let message;
    try {
      message = JSON.parse(line);
    } catch (err) {
      send({ jsonrpc: '2.0', id: null, error: { code: PARSE_ERROR, message: `parse error: ${/** @type {Error} */ (err).message}` } });
      return;
    }
    if (!message || typeof message !== 'object' || Array.isArray(message) || message.jsonrpc !== '2.0' || typeof message.method !== 'string') {
      // A response to something we sent (we send none that expect one) or junk.
      if (message && typeof message === 'object' && !Array.isArray(message) && ('result' in message || 'error' in message)) return;
      send({ jsonrpc: '2.0', id: message?.id ?? null, error: { code: INVALID_REQUEST, message: 'invalid request: expected a JSON-RPC 2.0 request object (batches are not supported)' } });
      return;
    }
    const isRequest = 'id' in message && message.id !== null && message.id !== undefined;
    try {
      const result = await handle(message.method, message.params ?? {}, message);
      if (isRequest) send({ jsonrpc: '2.0', id: message.id, result: result ?? null });
    } catch (err) {
      if (!(err instanceof RpcError) && !(err instanceof CommandError)) log(`internal error in ${message.method}: ${/** @type {Error} */ (err)?.stack ?? err}`);
      if (isRequest) send({ jsonrpc: '2.0', id: message.id, error: toRpcError(err) });
    }
  };

  lines.on('line', (line) => {
    chain = chain.then(() => one(line));
  });
  lines.on('close', () => {
    chain.then(() => resolveDone());
  });

  return {
    /** Sends a notification. */
    notify(/** @type {string} */ method, /** @type {any} */ params) {
      send({ jsonrpc: '2.0', method, params });
    },
    /** Resolves when the input ends and every request is answered. */
    done,
    close() {
      lines.close();
    },
    /** Stops writing (after shutdown). */
    stop() {
      closed = true;
    },
  };
}

/** Sends console output to stderr, so stdout carries only JSON-RPC. */
export function consoleToStderr() {
  const write = (/** @type {unknown[]} */ args) => process.stderr.write(`${args.map((a) => (typeof a === 'string' ? a : JSON.stringify(a))).join(' ')}\n`);
  console.log = (...args) => write(args);
  console.info = (...args) => write(args);
  console.debug = (...args) => write(args);
  console.warn = (...args) => write(args);
}
