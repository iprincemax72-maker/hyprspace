import { h } from "./dom.js";

export function markdown(src) {
  const lines = src.replace(/\r\n/g, "\n").split("\n");
  const out = h("div", { class: "md" });
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const fence = line.match(/^\s*(```|~~~)\s*([^\s`]*)/);
    if (fence) {
      const body = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith(fence[1])) body.push(lines[i++]);
      i++;
      out.append(code(fence[2], body.join("\n")));
      continue;
    }
    if (!line.trim()) {
      i++;
      continue;
    }
    const head = line.match(/^(#{1,6})\s+(.*)$/);
    if (head) {
      out.append(h(`h${Math.min(head[1].length + 2, 6)}`, null, inline(head[2])));
      i++;
      continue;
    }
    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      out.append(h("hr"));
      i++;
      continue;
    }
    if (/^\s*\|/.test(line) && i + 1 < lines.length && /^\s*\|?\s*:?-+/.test(lines[i + 1])) {
      const rows = [];
      while (i < lines.length && /^\s*\|/.test(lines[i])) rows.push(lines[i++]);
      out.append(table(rows));
      continue;
    }
    if (/^\s*>/.test(line)) {
      const body = [];
      while (i < lines.length && /^\s*>/.test(lines[i])) body.push(lines[i++].replace(/^\s*>\s?/, ""));
      out.append(h("blockquote", null, markdown(body.join("\n"))));
      continue;
    }
    const item = /^(\s*)([-*+]|\d+[.)])\s+/;
    if (item.test(line)) {
      const ordered = /\d/.test(line.match(item)[2]);
      const start = ordered ? parseInt(line.match(item)[2], 10) : null;
      const list = h(ordered ? "ol" : "ul", start && start !== 1 ? { start } : null);
      const base = line.match(item)[1].length;
      while (i < lines.length) {
        const m = lines[i].match(item);
        if (!m || m[1].length !== base) break;
        const body = [lines[i].slice(m[0].length)];
        i++;
        while (i < lines.length && lines[i].trim() && !(lines[i].match(item) && lines[i].match(item)[1].length <= base)) {
          body.push(lines[i].replace(/^\s{0,4}/, ""));
          i++;
        }
        while (i < lines.length && !lines[i].trim() && i + 1 < lines.length && /^\s{2,}\S/.test(lines[i + 1])) i++;
        list.append(h("li", null, body.length > 1 ? markdown(body.join("\n")) : inline(body[0])));
      }
      out.append(list);
      continue;
    }
    const para = [];
    while (i < lines.length && lines[i].trim() && !/^(#{1,6}\s|\s*(```|~~~)|\s*>|\s*([-*+]|\d+[.)])\s+|\s*\|)/.test(lines[i])) para.push(lines[i++]);
    if (!para.length) para.push(lines[i++]);
    out.append(h("p", null, inline(para.join(" "))));
  }
  return out;
}

function code(lang, text) {
  return h("div", { class: "code" }, lang ? h("div", { class: "code-lang" }, lang) : null, h("pre", null, h("code", null, text)));
}

function table(rows) {
  const cells = (r) => r.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
  const head = cells(rows[0]);
  const body = rows.slice(2).map(cells);
  return h("div", { class: "table" }, h("table", null, h("thead", null, h("tr", null, head.map((c) => h("th", null, inline(c))))), h("tbody", null, body.map((r) => h("tr", null, r.map((c) => h("td", null, inline(c))))))));
}

const INLINE = /(`+)([\s\S]*?)\1|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*\s][^*]*)\*|_([^_\s][^_]*)_|~~([^~]+)~~|\[([^\]]+)\]\(([^)\s]+)\)|(https?:\/\/[^\s<>()]+[^\s<>().,;:!?"'])/;

export function inline(text) {
  const out = [];
  let rest = text;
  while (rest) {
    const m = rest.match(INLINE);
    if (!m) {
      out.push(rest);
      break;
    }
    if (m.index) out.push(rest.slice(0, m.index));
    if (m[1]) out.push(h("code", null, m[2]));
    else if (m[3] || m[4]) out.push(h("strong", null, inline(m[3] || m[4])));
    else if (m[5] || m[6]) out.push(h("em", null, inline(m[5] || m[6])));
    else if (m[7]) out.push(h("s", null, inline(m[7])));
    else if (m[8]) out.push(link(m[9], inline(m[8])));
    else out.push(link(m[10], m[10]));
    rest = rest.slice(m.index + m[0].length);
  }
  return out;
}

function link(url, kids) {
  if (!/^https?:\/\//i.test(url)) return h("span", null, kids);
  return h("a", { href: url, target: "_blank", rel: "noopener noreferrer" }, kids);
}
