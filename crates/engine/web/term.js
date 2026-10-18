import { h, rgba } from "./dom.js";

const BOLD = 1;
const ITALIC = 2;
const UNDERLINE = 4;
const INVERSE = 8;
const DIM = 16;
const STRIKE = 32;
const HIDDEN = 64;
const RGB = 1 << 24;

const KEYS = {
  Enter: "\r",
  Backspace: "\x7f",
  Delete: "\x1b[3~",
  Escape: "\x1b",
  ArrowUp: "\x1b[A",
  ArrowDown: "\x1b[B",
  ArrowRight: "\x1b[C",
  ArrowLeft: "\x1b[D",
  Home: "\x1b[H",
  End: "\x1b[F",
  PageUp: "\x1b[5~",
  PageDown: "\x1b[6~",
};

export class Term {
  constructor(app, id) {
    this.app = app;
    this.id = id;
    this.lines = [];
    this.paste = false;
    this.follow = true;
    this.size = null;
    this.screen = h("div", { class: "screen" });
    this.cursor = h("div", { class: "cursor" });
    this.probe = h("span", { class: "probe" }, "MMMMMMMMMM");
    this.keys = h("textarea", {
      class: "keys",
      autocapitalize: "off",
      autocomplete: "off",
      spellcheck: false,
      onkeydown: (e) => this.key(e),
      oninput: () => this.typed(),
      onpaste: (e) => {
        e.preventDefault();
        const text = e.clipboardData.getData("text").replace(/\r?\n/g, "\r");
        if (text) this.app.send({ type: this.paste ? "paste" : "keys", thread: this.id, text });
      },
    });
    this.body = h("div", { class: "term-scroll", onscroll: () => this.scrolled(), onclick: () => this.focus() }, this.screen, this.cursor, this.probe);
    this.el = h("div", { class: "term" }, this.body, this.keys);
    this.fitter = new ResizeObserver(() => this.fit());
    this.fitter.observe(this.body);
  }

  focus() {
    if (!getSelection().toString()) this.keys.focus({ preventScroll: true });
  }

  leave() {
    this.fitter.disconnect();
    this.app.send({ type: "unfit", thread: this.id });
  }

  cell() {
    const r = this.probe.getBoundingClientRect();
    return [r.width / 10 || 8, r.height || 17];
  }

  fit() {
    const [w, lh] = this.cell();
    const cols = Math.max(20, Math.floor((this.body.clientWidth - 24) / w));
    const rows = Math.max(5, Math.floor((this.body.clientHeight - 16) / lh));
    const key = `${cols}x${rows}`;
    if (key === this.size) return;
    this.size = key;
    this.app.send({ type: "fit", thread: this.id, cols, rows });
  }

  frame(f) {
    if (f.reset) {
      this.lines = [];
      this.screen.replaceChildren();
    }
    const drop = Math.min(f.drop, this.lines.length);
    for (let i = 0; i < drop; i++) this.lines[i].remove();
    this.lines.splice(0, drop);
    while (this.lines.length > f.len) this.lines.pop().remove();
    while (this.lines.length < f.len) {
      const el = h("div", { class: "line" });
      this.screen.append(el);
      this.lines.push(el);
    }
    for (const [i, spans] of f.lines) if (this.lines[i]) this.lines[i].replaceChildren(...spans.map((s) => this.span(s)));
    this.paste = f.paste;
    const [w, lh] = this.cell();
    if (f.cursor) {
      this.cursor.style.display = "block";
      this.cursor.style.transform = `translate(${f.cursor[1] * w}px, ${f.cursor[0] * lh}px)`;
      this.cursor.style.width = `${w}px`;
      this.cursor.style.height = `${lh}px`;
    } else this.cursor.style.display = "none";
    if (this.follow) this.body.scrollTop = this.body.scrollHeight;
  }

  span(s) {
    const el = document.createElement("span");
    el.textContent = s.t;
    const st = s.s || 0;
    let fg = s.fg != null ? this.color(s.fg) : null;
    let bg = s.bg != null ? this.color(s.bg) : null;
    if (st & INVERSE) [fg, bg] = [bg || "var(--term-bg)", fg || "var(--term-fg)"];
    if (fg) el.style.color = fg;
    if (bg) el.style.background = bg;
    if (st & BOLD) el.style.fontWeight = "700";
    if (st & ITALIC) el.style.fontStyle = "italic";
    if (st & DIM) el.style.opacity = "0.6";
    if (st & HIDDEN) el.style.visibility = "hidden";
    const deco = [st & UNDERLINE ? "underline" : "", st & STRIKE ? "line-through" : ""].filter(Boolean).join(" ");
    if (deco) el.style.textDecoration = deco;
    return el;
  }

  color(c) {
    if (c & RGB) return `#${(c & 0xffffff).toString(16).padStart(6, "0")}`;
    if (c < 16) {
      const ansi = this.app.palette().ansi;
      return ansi && ansi[c] != null ? rgba(ansi[c]) : null;
    }
    if (c < 232) {
      const n = c - 16;
      const v = (x) => (x ? 55 + x * 40 : 0);
      return `rgb(${v(Math.floor(n / 36))}, ${v(Math.floor(n / 6) % 6)}, ${v(n % 6)})`;
    }
    const g = 8 + (c - 232) * 10;
    return `rgb(${g}, ${g}, ${g})`;
  }

  scrolled() {
    const b = this.body;
    this.follow = b.scrollHeight - b.scrollTop - b.clientHeight < 30;
  }

  send(text) {
    this.follow = true;
    this.app.send({ type: "keys", thread: this.id, text });
  }

  key(e) {
    if (e.isComposing) return;
    let text = KEYS[e.key];
    if (e.key === "Tab") text = e.shiftKey ? "\x1b[Z" : "\t";
    if (!text && e.ctrlKey && !e.altKey && !e.metaKey && e.key.length === 1) {
      const c = e.key.toLowerCase();
      if ((c === "c" && getSelection().toString()) || c === "v") return;
      if (c >= "a" && c <= "z") text = String.fromCharCode(c.charCodeAt(0) - 96);
      else if (c === " " || c === "@") text = "\x00";
      else if (c === "[") text = "\x1b";
      else if (c === "\\") text = "\x1c";
      else if (c === "]") text = "\x1d";
    }
    if (!text && e.altKey && !e.ctrlKey && e.key.length === 1) text = `\x1b${e.key}`;
    if (!text) return;
    e.preventDefault();
    this.send(text);
  }

  typed() {
    const text = this.keys.value;
    this.keys.value = "";
    if (text) this.send(text.replace(/\r?\n/g, "\r"));
  }
}
