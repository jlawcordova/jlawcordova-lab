// Pixel Lab's Agent chat (docs/intents/2026-10-agent-chat/spec.md): the app
// runs the owner's Claude Code with this CLI's MCP server, and these commands
// are the chat's own. chat.context builds the selection block a message
// carries (D5): what's selected, as the saved document has it. chat.ask puts
// a question with answer buttons in front of the owner (D8): it returns at
// once, and the owner's choice comes back as their next message.

import { importSource, readSource } from '../session.mjs';
import { objectContext, selectionContext } from '../chat-context.mjs';
import { UsageError } from '../errors.mjs';

const NAME = { type: 'string', description: 'scene name (or scenes/<name>)' };

/** The doc's saved file: its data and the version the block names (the first 8 hex digits of its hash). */
async function savedDoc(/** @type {any} */ ctx, /** @type {'scene' | 'object'} */ kind, /** @type {string} */ name) {
  const disk = await readSource(ctx.site, kind, name);
  if (!disk.exists) return { data: { items: [] }, version: 'unsaved' };
  return { data: await importSource(disk.text), version: disk.hash.slice(0, 8) };
}

/** The document's name for the block: its manifest's, else its folder's. */
const documentName = (/** @type {any} */ ctx) => String(ctx.site.name ?? 'document');

/** @type {import('../registry.mjs').Command[]} */
export default [
  {
    name: 'chat.context',
    summary: "Build the selection block an Agent chat message carries: what's selected, as the saved document has it",
    description:
      'Pixel Lab sends this block with each chat message. Layers and items are named as the app has them (its working copy, which may have unsaved edits); ' +
      'the block names them by their paths in the saved file. An item placed, moved or changed since the last save, or a layer made or changed since, ' +
      "isn't in the saved file: it's left out and listed in unsaved. With --object, the block names a whole object instead.",
    params: {
      type: 'object',
      properties: {
        name: NAME,
        object: { type: 'string', description: 'an object: the block names the whole object (object mode) instead of a scene' },
        layers: { type: 'array', items: { type: 'string' }, description: 'selected layer ids, comma-separated on the command line' },
        items: { type: 'array', items: { type: 'string' }, description: 'selected item paths (12, 5.2), comma-separated on the command line' },
        pinned: { type: 'array', items: { type: 'string' }, description: 'which of those are pinned to the chat: layer:<id> or a path' },
      },
    },
    args: ['name'],
    mutates: false,
    async run(ctx, { name, object, layers = [], items = [], pinned = [] }) {
      if (object) {
        const { name: found } = await ctx.find(object, 'object');
        const { version } = await savedDoc(ctx, 'object', found);
        return objectContext({ document: documentName(ctx), object: found, version });
      }
      if (!name) throw new UsageError('chat.context needs a scene name, or --object');
      const { name: scene } = await ctx.find(name, 'scene');
      const working = await ctx.doc('scene', scene);
      const saved = await savedDoc(ctx, 'scene', scene);
      return selectionContext({ document: documentName(ctx), scene, version: saved.version, working, saved: saved.data, layers, items, pinned });
    },
    format: (r) => r.text + (r.unsaved.length ? `\nleft out (not saved): ${r.unsaved.join(', ')}` : ''),
    examples: ['pixellab chat context harbor --items 45.0,46.0 --layers boats'],
  },
  {
    name: 'chat.ask',
    summary: 'Ask the owner a question with two to four answers, shown as buttons in Pixel Lab\'s Agent chat',
    description:
      'Use it when the owner should choose (every use of an object or only these, one way or another). It returns at once: the question shows in the chat with a button per answer, ' +
      'and the owner\'s choice arrives as their next message, so end your turn after asking. With no Pixel Lab chat on this connection (a terminal), it says so: ask in your own conversation.',
    params: {
      type: 'object',
      properties: {
        question: { type: 'string', description: 'the question, in one or two sentences' },
        options: { type: 'array', items: { type: 'string' }, description: 'two to four short answers, as button labels' },
      },
      required: ['question', 'options'],
    },
    args: ['question'],
    mutates: false,
    run(ctx, { question, options }) {
      const q = String(question).trim();
      const answers = (options ?? []).map((/** @type {any} */ o) => String(o).trim()).filter(Boolean);
      if (!q) throw new UsageError('chat.ask needs a question');
      if (answers.length < 2 || answers.length > 4) throw new UsageError(`chat.ask needs two to four answers (got ${answers.length})`);
      if (answers.some((/** @type {string} */ a) => a.length > 60)) throw new UsageError('each answer is a button label: 60 characters at most');
      const chat = ctx.session?.chat ?? null;
      return chat
        ? { asked: true, question: q, options: answers, chat, note: 'Pixel Lab shows your question with these answers as buttons. Their choice arrives as their next message. End your turn now without doing more.' }
        : { asked: false, question: q, options: answers, note: 'No Pixel Lab chat is attached to this connection: ask in your own conversation.' };
    },
    format: (r) => `${r.question}\n${r.options.map((/** @type {string} */ o, /** @type {number} */ i) => `  ${i + 1}. ${o}`).join('\n')}${r.asked ? '' : `\n(${r.note})`}`,
    examples: ['pixellab chat ask "Make every pine taller, or only these 2?" --options "Only these 2,Every pine (8)"'],
  },
];
