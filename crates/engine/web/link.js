const PROTOCOL = 2;
const TOKEN = "hyprspace.token";

function stored() {
  try {
    return localStorage.getItem(TOKEN);
  } catch {
    return null;
  }
}

function keep(token) {
  try {
    if (token) localStorage.setItem(TOKEN, token);
    else localStorage.removeItem(TOKEN);
  } catch {}
}

function device() {
  const ua = navigator.userAgent;
  const browser = /Edg\//.test(ua) ? "Edge" : /Firefox\//.test(ua) ? "Firefox" : /Chrome\//.test(ua) ? "Chrome" : /Safari\//.test(ua) ? "Safari" : "Browser";
  const os = /Windows/.test(ua) ? "Windows" : /Android/.test(ua) ? "Android" : /iPhone|iPad/.test(ua) ? "iOS" : /Mac OS/.test(ua) ? "macOS" : /Linux/.test(ua) ? "Linux" : "";
  return os ? `${browser} on ${os}` : browser;
}

async function prove(code) {
  const key = code.replace(/[^a-z0-9]/gi, "").toUpperCase();
  const enc = new TextEncoder();
  const k = await crypto.subtle.importKey("raw", enc.encode(key), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const sig = new Uint8Array(await crypto.subtle.sign("HMAC", k, enc.encode("browser")));
  return btoa(String.fromCharCode(...sig)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export class Link {
  constructor(on) {
    this.on = on;
    this.ws = null;
    this.wait = 1000;
    this.retry = null;
    this.pinger = null;
    this.state = stored() ? "connecting" : "unpaired";
  }

  paired() {
    return !!stored();
  }

  start() {
    if (stored()) this.open({ type: "hello", token: stored(), device: device(), protocol: PROTOCOL, app: "browser" });
  }

  async pair(code) {
    this.open({ type: "pair", proof: await prove(code), device: device(), protocol: PROTOCOL, app: "browser" });
  }

  forget() {
    this.send({ type: "leave" });
    keep(null);
    this.close();
    this.status("unpaired");
  }

  open(first) {
    this.close();
    clearTimeout(this.retry);
    const ws = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`);
    this.ws = ws;
    this.status("connecting");
    let denied = false;
    ws.onopen = () => ws.send(JSON.stringify(first));
    ws.onmessage = (e) => {
      if (typeof e.data !== "string") return;
      const down = JSON.parse(e.data);
      if (down.type === "welcome") {
        if (down.token) keep(down.token);
        this.wait = 1000;
        this.status("online");
        clearInterval(this.pinger);
        this.pinger = setInterval(() => this.send({ type: "ping" }), 20000);
      } else if (down.type === "denied") {
        denied = true;
        if (down.forget) keep(null);
        this.status(stored() ? "denied" : "unpaired", down.message);
      }
      this.on(down);
    };
    ws.onclose = () => {
      clearInterval(this.pinger);
      if (this.ws !== ws) return;
      this.ws = null;
      if (denied || !stored()) return;
      this.status("offline");
      this.retry = setTimeout(() => this.start(), this.wait);
      this.wait = Math.min(this.wait * 2, 10000);
    };
  }

  close() {
    const ws = this.ws;
    this.ws = null;
    clearInterval(this.pinger);
    if (ws) ws.close();
  }

  status(state, message) {
    this.state = state;
    this.message = message;
    this.on({ type: "link", state, message });
  }

  send(up) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(up));
      return true;
    }
    return false;
  }
}
