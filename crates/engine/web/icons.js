import { h } from "./dom.js";

export function icon(name, size = 14, cls = "") {
  return h("span", { class: `icon ${cls}`, style: { width: `${size}px`, height: `${size}px`, maskImage: `url(/icons/${name}.svg)`, webkitMaskImage: `url(/icons/${name}.svg)` } });
}

export function mark(agent, size = 13) {
  if (agent === "claude") return h("img", { class: "mark", src: "/brand/claude.svg", width: size, height: size, alt: "" });
  if (agent === "codex") return h("span", { class: "icon mark-codex", style: { width: `${size}px`, height: `${size}px`, maskImage: "url(/brand/openai.svg)", webkitMaskImage: "url(/brand/openai.svg)" } });
  return icon("terminal", size);
}
