// A small Markdown renderer for the docs site: the CommonMark and GitHub
// constructs the repo's docs use, with Node built-ins only.
//
//   blocks   ATX headings, paragraphs, fenced code, block quotes, bullet and
//            ordered lists (nested, holding any block, task items), GFM tables
//            (`\|` in a cell is a pipe), thematic breaks; HTML comments are dropped
//   inline   code spans, **strong**, *em* and _em_, [links](url "title"),
//            ![images](src), <https://autolinks>, bare https:// URLs, backslash
//            escapes, hard breaks
//
// Raw HTML is never passed through: it's shown as text. Heading ids follow
// GitHub's (lowercase, punctuation dropped, spaces to hyphens, -1, -2 for
// repeats), so anchors that work on GitHub work on the site.

/** @typedef {{ level: number, text: string, id: string }} Heading */
/**
 * @typedef {{
 *   link?: (href: string, kind: 'link' | 'image') => string,
 *   slugger?: (text: string) => string,
 *   inLink?: boolean,
 * }} Options
 */

const ESCAPABLE = /[!"#$%&'()*+,\-./:;<=>?@[\\\]^_`{|}~]/;

export function escapeHtml(/** @type {string} */ text) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/** Text with entities kept (&amp; &#169; &copy;), everything else escaped. */
function escapeText(/** @type {string} */ text) {
  return text
    .replace(/&(?!#\d+;|#x[0-9a-f]+;|[a-z][a-z0-9]*;)/gi, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

const ENTITIES = /** @type {Record<string, string>} */ ({ amp: '&', lt: '<', gt: '>', quot: '"', '#39': "'", nbsp: ' ' });

/** The text of rendered inline HTML: tags dropped, entities decoded. */
export function textOf(/** @type {string} */ html) {
  return html
    .replace(/<[^>]*>/g, '')
    .replace(/&(#x[0-9a-f]+|#\d+|[a-z0-9#]+);/gi, (m, e) => {
      if (e[0] === '#') return String.fromCodePoint(e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10));
      return ENTITIES[e] ?? m;
    });
}

/** GitHub's heading ids: one slugger per page, so repeats get -1, -2. */
export function createSlugger() {
  /** @type {Map<string, number>} */
  const seen = new Map();
  return (/** @type {string} */ text) => {
    const base = text.trim().toLowerCase().replace(/[^\p{L}\p{M}\p{N}\p{Pc}\- ]/gu, '').replace(/ /g, '-');
    let slug = base;
    while (seen.has(slug)) {
      const n = /** @type {number} */ (seen.get(base)) + 1;
      seen.set(base, n);
      slug = `${base}-${n}`;
    }
    seen.set(slug, 0);
    return slug;
  };
}

// ---------------------------------------------------------------- inline

/**
 * Inline Markdown to HTML. Code spans, links, images and autolinks become
 * placeholders first, so emphasis can wrap them (**`pixellab`**) without
 * reaching inside them.
 * @param {string} src
 * @param {Options} opts
 */
export function renderInline(src, opts = {}) {
  const link = opts.link ?? ((h) => h);
  /** @type {string[]} */
  const held = [];
  const hold = (/** @type {string} */ html) => `\u0001${held.push(html) - 1}\u0002`;
  let out = '';
  let i = 0;
  let plain = '';
  const flush = () => {
    out += escapeText(plain);
    plain = '';
  };

  while (i < src.length) {
    const c = src[i];
    if (c === '\\' && i + 1 < src.length && ESCAPABLE.test(src[i + 1])) {
      flush();
      out += hold(escapeHtml(src[i + 1]));
      i += 2;
      continue;
    }
    if (c === '\\' && src[i + 1] === '\n') {
      flush();
      out += hold('<br>\n');
      i += 2;
      continue;
    }
    if (c === '`') {
      const run = /^`+/.exec(src.slice(i))[0];
      const close = findCodeClose(src, i + run.length, run.length);
      if (close >= 0) {
        flush();
        let code = src.slice(i + run.length, close).replace(/\n/g, ' ');
        if (code.length > 2 && code.startsWith(' ') && code.endsWith(' ') && code.trim()) code = code.slice(1, -1);
        out += hold(`<code>${escapeHtml(code)}</code>`);
        i = close + run.length;
        continue;
      }
      plain += run;
      i += run.length;
      continue;
    }
    if (c === '<' && src.startsWith('<!--', i)) {
      const end = src.indexOf('-->', i + 4);
      if (end >= 0) {
        flush();
        i = end + 3;
        continue;
      }
    }
    if (c === '<') {
      const m = /^<((?:https?|mailto):[^\s<>]*)>/i.exec(src.slice(i));
      if (m) {
        flush();
        out += hold(`<a href="${escapeHtml(link(m[1], 'link'))}">${escapeHtml(m[1])}</a>`);
        i += m[0].length;
        continue;
      }
    }
    if ((c === '!' && src[i + 1] === '[') || c === '[') {
      const image = c === '!';
      const parsed = parseLink(src, image ? i + 1 : i);
      if (parsed) {
        flush();
        const title = parsed.title ? ` title="${escapeHtml(parsed.title)}"` : '';
        if (image) {
          const alt = textOf(renderInline(parsed.text, opts));
          out += hold(`<img src="${escapeHtml(link(parsed.href, 'image'))}" alt="${escapeHtml(alt)}"${title} loading="lazy">`);
        } else {
          out += hold(`<a href="${escapeHtml(link(parsed.href, 'link'))}"${title}>${renderInline(parsed.text, { ...opts, inLink: true })}</a>`);
        }
        i = parsed.end;
        continue;
      }
    }
    if (c === 'h' && !opts.inLink && /^https?:\/\//.test(src.slice(i, i + 8)) && !/[\w/]$/.test(plain)) {
      let url = /^https?:\/\/[^\s<>]+/.exec(src.slice(i))[0];
      url = trimUrl(url);
      flush();
      out += hold(`<a href="${escapeHtml(link(url, 'link'))}">${escapeHtml(url)}</a>`);
      i += url.length;
      continue;
    }
    if (c === ' ' && src.startsWith('  \n', i)) {
      flush();
      out += hold('<br>\n');
      i += 3;
      continue;
    }
    plain += c;
    i += 1;
  }
  flush();

  out = emphasis(out);
  return out.replace(/\u0001(\d+)\u0002/g, (_, n) => held[Number(n)]);
}

function findCodeClose(/** @type {string} */ src, /** @type {number} */ from, /** @type {number} */ len) {
  let j = from;
  while (j < src.length) {
    const k = src.indexOf('`', j);
    if (k < 0) return -1;
    const run = /^`+/.exec(src.slice(k))[0].length;
    if (run === len) return k;
    j = k + run;
  }
  return -1;
}

/** A URL found in running text, without the punctuation that ends a sentence. */
function trimUrl(/** @type {string} */ url) {
  for (;;) {
    const last = url[url.length - 1];
    if (/[.,:;!?'"*_]/.test(last)) url = url.slice(0, -1);
    else if (last === ')' && (url.match(/\(/g) ?? []).length < (url.match(/\)/g) ?? []).length) url = url.slice(0, -1);
    else return url;
  }
}

/**
 * `[text](href "title")` starting at src[at] === '['; null if it isn't one.
 * @returns {{ text: string, href: string, title: string | null, end: number } | null}
 */
function parseLink(/** @type {string} */ src, /** @type {number} */ at) {
  let depth = 0;
  let j = at;
  for (; j < src.length; j++) {
    const ch = src[j];
    if (ch === '\\') {
      j++;
      continue;
    }
    if (ch === '`') {
      const run = /^`+/.exec(src.slice(j))[0].length;
      const close = findCodeClose(src, j + run, run);
      if (close >= 0) {
        j = close + run - 1;
        continue;
      }
    }
    if (ch === '[') depth++;
    else if (ch === ']' && --depth === 0) break;
  }
  if (j >= src.length || src[j + 1] !== '(') return null;
  const text = src.slice(at + 1, j);
  let k = j + 2;
  while (src[k] === ' ') k++;
  let href = '';
  if (src[k] === '<') {
    const close = src.indexOf('>', k);
    if (close < 0) return null;
    href = src.slice(k + 1, close);
    k = close + 1;
  } else {
    let parens = 0;
    for (; k < src.length; k++) {
      const ch = src[k];
      if (ch === '\\' && k + 1 < src.length) {
        href += src[++k];
        continue;
      }
      if (ch === '(') parens++;
      else if (ch === ')') {
        if (parens === 0) break;
        parens--;
      } else if (/\s/.test(ch)) break;
      href += ch;
    }
  }
  while (src[k] === ' ') k++;
  let title = null;
  if (src[k] === '"' || src[k] === "'") {
    const q = src[k];
    const close = src.indexOf(q, k + 1);
    if (close < 0) return null;
    title = src.slice(k + 1, close);
    k = close + 1;
    while (src[k] === ' ') k++;
  }
  if (src[k] !== ')') return null;
  return { text, href, title, end: k + 1 };
}

/** Strong and em over escaped text with placeholders (which hold no * or _). */
function emphasis(/** @type {string} */ s) {
  s = s.replace(/\*\*(?=\S)([\s\S]*?\S)\*\*/g, '<strong>$1</strong>');
  s = s.replace(/(^|[^\w*])\*(?=[^\s*])([\s\S]*?[^\s*])\*(?![\w*])/g, '$1<em>$2</em>');
  s = s.replace(/(^|[^\w])_(?=[^\s_])([\s\S]*?[^\s_])_(?!\w)/g, '$1<em>$2</em>');
  return s;
}

// ---------------------------------------------------------------- blocks

const FENCE = /^( {0,3})(`{3,}|~{3,})\s*([^`\s]*)[^`]*$/;
const HEADING = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/;
const RULE = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
const QUOTE = /^ {0,3}> ?/;
const LIST = /^( {0,3})([-*+]|\d{1,9}[.)])( +|\t|$)/;

const indentOf = (/** @type {string} */ line) => /^ */.exec(line.replace(/\t/g, '    '))[0].length;
const isBlank = (/** @type {string} */ line) => /^\s*$/.test(line);

/** Removes up to n columns of leading spaces. */
function dedent(/** @type {string} */ line, /** @type {number} */ n) {
  line = line.replace(/^\t+/, (t) => '    '.repeat(t.length));
  let k = 0;
  while (k < n && line[k] === ' ') k++;
  return line.slice(k);
}

function splitRow(/** @type {string} */ line) {
  let s = line.trim();
  if (s.startsWith('|')) s = s.slice(1);
  if (s.endsWith('|') && !s.endsWith('\\|')) s = s.slice(0, -1);
  /** @type {string[]} */
  const cells = [];
  let cell = '';
  for (let k = 0; k < s.length; k++) {
    if (s[k] === '\\' && s[k + 1] === '|') {
      cell += '|';
      k++;
    } else if (s[k] === '|') {
      cells.push(cell.trim());
      cell = '';
    } else cell += s[k];
  }
  cells.push(cell.trim());
  return cells;
}

/** A table's delimiter row: | --- | :---: | ---: | */
function isDelimiterRow(/** @type {string} */ line) {
  return line.includes('|') && line.includes('-') && splitRow(line).every((c) => /^:?-+:?$/.test(c));
}

/** Does this line start a block that ends a paragraph? */
function interrupts(/** @type {string} */ line) {
  return FENCE.test(line) || HEADING.test(line) || RULE.test(line) || QUOTE.test(line) || /^ {0,3}<!--/.test(line) || (LIST.test(line) && !isBlank(line.replace(LIST, '')));
}

/**
 * Renders Markdown to HTML, collecting the headings.
 * @param {string} src
 * @param {Options} [opts]
 * @returns {{ html: string, title: string | null, headings: Heading[] }}
 */
export function renderMarkdown(src, opts = {}) {
  const slug = opts.slugger ?? createSlugger();
  /** @type {Heading[]} */
  const headings = [];
  const inline = (/** @type {string} */ text) => renderInline(text, opts);

  /** @param {string[]} lines @param {boolean} tight */
  function blocks(lines, tight = false) {
    /** @type {string[]} */
    const out = [];
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      if (isBlank(line)) {
        i++;
        continue;
      }

      let m = FENCE.exec(line);
      if (m) {
        const [, indent, fence, lang] = m;
        /** @type {string[]} */
        const body = [];
        i++;
        while (i < lines.length && !new RegExp(`^ {0,3}${fence[0] === '`' ? '`' : '~'}{${fence.length},}\\s*$`).test(lines[i])) {
          body.push(dedent(lines[i], indent.length));
          i++;
        }
        i++;
        const cls = lang ? ` class="language-${escapeHtml(lang)}"` : '';
        out.push(`<pre><code${cls}>${escapeHtml(body.join('\n'))}${body.length ? '\n' : ''}</code></pre>`);
        continue;
      }

      if (/^ {0,3}<!--/.test(line)) {
        while (i < lines.length && !lines[i].includes('-->')) i++;
        i++;
        continue;
      }

      m = HEADING.exec(line);
      if (m) {
        const level = m[1].length;
        const html = inline((m[2] ?? '').trim());
        const text = textOf(html).trim();
        const id = slug(text);
        headings.push({ level, text, id });
        out.push(`<h${level} id="${escapeHtml(id)}">${html}<a class="anchor" href="#${escapeHtml(id)}" aria-label="Link to ${escapeHtml(text)}">#</a></h${level}>`);
        i++;
        continue;
      }

      if (RULE.test(line)) {
        out.push('<hr>');
        i++;
        continue;
      }

      if (QUOTE.test(line)) {
        /** @type {string[]} */
        const body = [];
        while (i < lines.length && (QUOTE.test(lines[i]) || (!isBlank(lines[i]) && body.length && !isBlank(body[body.length - 1]) && !interrupts(lines[i])))) {
          body.push(lines[i].replace(QUOTE, ''));
          i++;
        }
        out.push(`<blockquote>\n${blocks(body)}\n</blockquote>`);
        continue;
      }

      m = LIST.exec(line);
      if (m) {
        const r = list(lines, i);
        out.push(r.html);
        i = r.end;
        continue;
      }

      if (line.includes('|') && i + 1 < lines.length && isDelimiterRow(lines[i + 1])) {
        const head = splitRow(line);
        const align = splitRow(lines[i + 1]).map((d) => (d.startsWith(':') && d.endsWith(':') ? 'center' : d.endsWith(':') ? 'right' : d.startsWith(':') ? 'left' : ''));
        i += 2;
        /** @type {string[][]} */
        const rows = [];
        while (i < lines.length && !isBlank(lines[i]) && lines[i].includes('|')) {
          rows.push(splitRow(lines[i]));
          i++;
        }
        const cell = (/** @type {string} */ tag, /** @type {string} */ text, /** @type {number} */ k) => `<${tag}${align[k] ? ` style="text-align:${align[k]}"` : ''}>${inline(text)}</${tag}>`;
        const thead = `<thead><tr>${head.map((t, k) => cell('th', t, k)).join('')}</tr></thead>`;
        const tbody = rows.length ? `<tbody>${rows.map((r) => `<tr>${head.map((_, k) => cell('td', r[k] ?? '', k)).join('')}</tr>`).join('\n')}</tbody>` : '';
        out.push(`<div class="table-wrap"><table>${thead}\n${tbody}</table></div>`);
        continue;
      }

      /** @type {string[]} */
      const para = [line.trim()];
      i++;
      while (i < lines.length && !isBlank(lines[i]) && !interrupts(lines[i])) {
        para.push(lines[i].replace(/^\s+/, ''));
        i++;
      }
      const text = inline(para.join('\n').replace(/\s+$/, ''));
      out.push(tight ? text : `<p>${text}</p>`);
    }
    return out.join('\n');
  }

  /** @param {string[]} lines @param {number} start */
  function list(lines, start) {
    const first = /** @type {RegExpExecArray} */ (LIST.exec(lines[start]));
    const ordered = /\d/.test(first[2]);
    const delim = ordered ? first[2].slice(-1) : first[2];
    const base = first[1].length;
    /** @type {{ lines: string[], task: string | null }[]} */
    const items = [];
    let loose = false;
    let i = start;
    let sawBlank = false;
    while (i < lines.length) {
      const m = LIST.exec(lines[i]);
      const sameList = m && m[1].length <= base + 1 && (ordered ? /\d/.test(m[2]) && m[2].slice(-1) === delim : m[2] === delim);
      if (!sameList) break;
      if (sawBlank && items.length) loose = true;
      sawBlank = false;
      const markerWidth = m[1].length + m[2].length;
      const after = lines[i].slice(m[0].length);
      const gap = m[3] === '' ? 1 : m[3] === '\t' ? 4 : m[3].length;
      const contentIndent = markerWidth + (gap > 4 ? 1 : gap);
      /** @type {string[]} */
      const body = [gap > 4 ? ' '.repeat(gap - 1) + after : after];
      i++;
      while (i < lines.length) {
        const l = lines[i];
        if (isBlank(l)) {
          body.push('');
          i++;
          continue;
        }
        if (indentOf(l) >= contentIndent) {
          body.push(dedent(l, contentIndent));
          i++;
          continue;
        }
        const prev = body[body.length - 1];
        if (!isBlank(prev) && !interrupts(l) && !LIST.test(l)) {
          body.push(l.trim());
          i++;
          continue;
        }
        break;
      }
      while (body.length && isBlank(body[body.length - 1])) {
        body.pop();
        sawBlank = true;
      }
      // A blank line between two blocks of one item makes the list loose.
      for (let k = 1; k < body.length - 1; k++) if (isBlank(body[k]) && !isBlank(body[k + 1]) && indentOf(body[k + 1]) === 0) loose = true;
      const task = /^\[([ xX])\] /.exec(body[0]);
      if (task) body[0] = body[0].slice(4);
      items.push({ lines: body, task: task ? task[1] : null });
      if (sawBlank && !(i < lines.length && LIST.test(lines[i]))) break;
    }
    const startNum = ordered ? parseInt(first[2], 10) : 1;
    const tag = ordered ? 'ol' : 'ul';
    const attrs = ordered && startNum !== 1 ? ` start="${startNum}"` : '';
    const lis = items.map(({ lines: body, task }) => {
      const check = task === null ? '' : `<input type="checkbox" disabled${task === ' ' ? '' : ' checked'}> `;
      return `<li${task === null ? '' : ' class="task"'}>${check}${blocks(body, !loose)}</li>`;
    });
    return { html: `<${tag}${attrs}>\n${lis.join('\n')}\n</${tag}>`, end: i };
  }

  const html = blocks(src.replace(/\r\n?/g, '\n').split('\n'));
  const h1 = headings.find((h) => h.level === 1);
  return { html, title: h1 ? h1.text : null, headings };
}
