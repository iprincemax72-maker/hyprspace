import { ago, elapsed, h, rgba } from "./dom.js";
import { icon, mark } from "./icons.js";

export function sidebar(app) {
  const board = app.board;
  const spaces = new Map(board.spaces.map((s) => [s.id, s]));
  const q = app.search.trim().toLowerCase();
  const order = (a, b) => b.pinned - a.pinned || b.rank - a.rank || b.touched - a.touched;
  const shelf = (name) => board.threads.filter((t) => t.shelf === name && (!q || t.title.toLowerCase().includes(q))).sort(order);
  const active = shelf("active");
  const row = (t) => thread(app, t, spaces.get(t.space));
  const group = (name, list) =>
    list.length
      ? h(
          "div",
          { class: "shelf" },
          h("button", { class: "shelf-head", onclick: () => app.toggleShelf(name) }, h("span", null, name === "snoozed" ? "Snoozed" : "Settled"), h("span", { class: "count" }, String(list.length)), h("span", { class: "rule" }), h("span", { class: `chev${app.shelves.has(name) || q ? " open" : ""}` }, icon("chevron-right", 12))),
          app.shelves.has(name) || q ? list.map(row) : null,
        )
      : null;
  return h(
    "nav",
    { class: "sidebar" },
    h("div", { class: "side-top" }, h("label", { class: "search" }, icon("search", 14), app.searchBox), h("button", { class: "new", title: "New thread", onclick: () => app.go({ compose: true }) }, icon("plus", 15))),
    h("div", { class: "rows" }, active.length ? active.map(row) : h("div", { class: "empty" }, q ? "Nothing matches." : "No threads yet."), group("snoozed", shelf("snoozed")), group("settled", shelf("settled"))),
    h("div", { class: "side-foot" }, h("span", { class: `link ${app.link.state}` }, app.desktop && app.link.state === "online" ? app.desktop : linkText(app.link.state)), h("button", { class: "quiet-button", onclick: () => app.forget() }, "Sign out")),
  );
}

function linkText(state) {
  return { online: "Connected", connecting: "Connecting", offline: "Reconnecting", denied: "Not let in", unpaired: "Not paired" }[state] || state;
}

export function tag(space, dim) {
  if (!space) return null;
  const dark = document.documentElement.dataset.side === "dark";
  const [fill, ink] = dark ? [space.tag[2], space.tag[3]] : [space.tag[0], space.tag[1]];
  return h("span", { class: `tag${dim ? " dim" : ""}`, style: { background: fill != null ? rgba(fill) : null, color: ink != null ? rgba(ink) : null } }, initials(space.name));
}

function initials(name) {
  const words = name.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  if (!words.length) return "";
  if (words.length > 1) return (words[0][0] + words[1][0]).toUpperCase();
  const rest = [...words[0]].slice(1);
  return (words[0][0] + (rest.find((c) => /\d/.test(c)) || rest[rest.length - 1] || "")).toUpperCase();
}

function thread(app, t, space) {
  const on = app.route.thread === t.id;
  const working = t.status === "working";
  const waiting = t.status === "waiting";
  if (t.shelf !== "active") {
    return h("button", { class: `row slim${on ? " on" : ""}`, onclick: () => app.go({ thread: t.id }) }, tag(space, true), h("span", { class: "title" }, t.title || "New thread"), h("span", { class: "when" }, ago(t.touched)));
  }
  const right = working && t.since ? h("span", { class: "running", "data-since": t.since }, elapsed(t.since)) : h("span", { class: "when" }, ago(t.touched));
  const state = waiting ? h("span", { class: "dot waiting" }) : t.status === "failed" ? h("span", { class: "dot failed" }) : t.status === "done" && t.unseen ? h("span", { class: "done" }, icon("check", 9)) : null;
  const foot = t.doing && (working || waiting) ? h("span", { class: `doing${waiting ? " waiting" : ""}` }, t.doing) : h("span", { class: "place" }, icon(t.branch ? "git-branch" : "folder", 11), h("span", { class: "mono" }, t.place));
  return h(
    "button",
    { class: `row${on ? " on" : ""}${waiting && !on ? " asks" : ""}`, onclick: () => app.go({ thread: t.id }) },
    h("div", { class: "row-top" }, tag(space), h("span", { class: "space" }, space ? space.name : ""), right),
    h("div", { class: "title" }, t.title || "New thread"),
    h("div", { class: "row-foot" }, foot, state, h("span", { class: `agent${working ? " spin" : ""}` }, mark(t.agent, 13))),
  );
}
