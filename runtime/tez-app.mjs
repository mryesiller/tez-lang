// The Elm-architecture host for Tez apps: `init() -> Model`, `update(Model, Msg) -> Model`,
// `view(Model) -> Html[Msg]`. The program is pure; this file owns the DOM and the event loop.
//
// A view is read out of WebAssembly into plain vnodes, diffed against the previous vnodes, and only the
// differences touch the DOM. `Msg` values stay opaque WebAssembly references: the host never inspects them.

import { TezError } from "./tez-runtime.mjs";

/** Returns true when the compiled program has the three functions of an app. */
export function isApp(rt) {
  const f = rt.meta.fns;
  return Boolean(f.init && f.update && f.view && f.init.params === 0 && f.update.params === 2 && f.view.params === 1);
}

/** Ints that fit 31 bits are plain numbers in WebAssembly, larger ones BigInts (see tez-runtime.mjs). */
const wasmInt = (n) => (Math.abs(n) < 2 ** 30 ? n : BigInt(n));

/**
 * The active subscriptions of a model, from the optional `subs(model)` function:
 * `{kind: "every", ms, msg}` (a fixed message), `{kind: "clock", ms, fn}` (the wall clock in seconds is the
 * argument), `{kind: "online" | "visible", fn}` (a Bool argument, sent when the browser's state changes).
 */
export function readSubs(rt, model) {
  if (!rt.meta.fns.subs || rt.meta.fns.subs.params !== 1) return [];
  const ex = rt.exports;
  return ex.f_subs(model).map((s) => {
    switch (ex.__tag(s)) {
      case 0:
        return { kind: "every", ms: Math.max(50, Number(ex.__get0(s))), msg: ex.__get1(s) };
      case 1:
        return { kind: "clock", ms: Math.max(1000, Number(ex.__get0(s))), fn: ex.__get1(s) };
      case 2:
        return { kind: "online", fn: ex.__get0(s) };
      default:
        return { kind: "visible", fn: ex.__get0(s) };
    }
  });
}

/** The message a subscription delivers; `value` is the clock (seconds) or the Bool of a state change. */
export function subMessage(rt, sub, value) {
  if (sub.kind === "every") return sub.msg;
  return rt.exports.__apply1(sub.fn, sub.kind === "clock" ? wasmInt(value) : value ? 1 : 0);
}

/** Reads an `Html[Msg]` value out of WebAssembly into `{text}` / `{tag, props, click, input, kids}` vnodes. */
export function readHtml(rt, v) {
  const ex = rt.exports;
  if (ex.__tag(v) === 0) return { text: ex.__get0(v) };
  const tag = ex.__get0(v);
  const props = {};
  let click;
  let input;
  for (const a of ex.__get1(v)) {
    switch (ex.__tag(a)) {
      case 0:
        click = ex.__get0(a);
        break;
      case 1:
        input = ex.__get0(a);
        break;
      default:
        props[ex.__get0(a)] = ex.__get1(a);
    }
  }
  return { tag, props, click, input, kids: ex.__get2(v).map((k) => readHtml(rt, k)) };
}

// ------------------------------------------------------------------ sandbox
// A Tez program may come from a model or a stranger, and it must not reach the page's JavaScript: only inert
// elements and attributes are rendered. Anything else is dropped (an unknown tag becomes a <div>).
const SAFE_TAGS = new Set(
  ("div span p h1 h2 h3 h4 h5 h6 ul ol li button input textarea select option label section article header footer " +
    "nav main aside strong em b i u s small sub sup code pre blockquote table thead tbody tfoot tr td th caption " +
    "img a br hr details summary figure figcaption progress meter time mark abbr dl dt dd").split(" "),
);
const URL_ATTRS = new Set(["href", "src", "poster", "cite", "action", "formaction", "xlink:href", "srcset", "background"]);

/** The tag actually created for a Tez tag name. */
export function safeTag(tag) {
  const t = String(tag).toLowerCase();
  return SAFE_TAGS.has(t) ? t : "div";
}

/** The attribute value to set, or `null` to drop the attribute. */
export function safeAttr(name, value) {
  const n = String(name).toLowerCase();
  const v = String(value);
  if (!/^[a-z][a-z0-9-]*$/.test(n)) return null; // also rejects `xlink:href`, namespaced tricks
  if (n.startsWith("on") || n === "srcdoc" || n === "formaction" || n === "action" || n === "srcset") return null;
  if (URL_ATTRS.has(n)) {
    // only web, mail and page-relative links: no javascript:, data:, vbscript: ...
    const bare = v.replace(/[\u0000-\u0020\u007f]+/g, "");
    const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(bare);
    if (scheme && !["http", "https", "mailto"].includes(scheme[1].toLowerCase())) return null;
  }
  if (n === "style" && /url\s*\(|expression\s*\(|@import|javascript:/i.test(v)) return null;
  return v;
}

function create(doc, vn, fire) {
  if (vn.text !== undefined) return doc.createTextNode(vn.text);
  const el = doc.createElement(safeTag(vn.tag));
  setProps(el, {}, vn);
  bind(el, vn, fire);
  for (const k of vn.kids) el.appendChild(create(doc, k, fire));
  return el;
}

function setProps(el, old, vn) {
  for (const k of Object.keys(old.props ?? {})) {
    if (!(k in vn.props) && safeAttr(k, "") !== null) el.removeAttribute(k);
  }
  for (const [k, v] of Object.entries(vn.props)) {
    if (old.props?.[k] === v) continue;
    const ok = safeAttr(k, v);
    if (k === "value" && "value" in el) {
      if (el.value !== v) el.value = v;
    } else if (ok === null) {
      el.removeAttribute(k);
    } else {
      el.setAttribute(k, ok);
    }
  }
}

// Handlers read the *current* vnode from the element, so a patch only has to swap `el.__tez`.
function bind(el, vn, fire) {
  el.__tez = vn;
  if (el.__bound) return;
  el.__bound = true;
  el.addEventListener("click", () => fire(el, "click"));
  el.addEventListener("input", () => fire(el, "input", el.value));
}

function patch(doc, parent, el, old, vn, fire) {
  if (!el) {
    parent.appendChild(create(doc, vn, fire));
    return;
  }
  const same = old.text !== undefined ? vn.text !== undefined : vn.text === undefined && safeTag(old.tag) === safeTag(vn.tag);
  if (!same) {
    parent.replaceChild(create(doc, vn, fire), el);
    return;
  }
  if (vn.text !== undefined) {
    if (old.text !== vn.text) el.nodeValue = vn.text;
    return;
  }
  setProps(el, old, vn);
  bind(el, vn, fire);
  for (let i = vn.kids.length; i < old.kids.length; i++) el.removeChild(el.lastChild);
  for (let i = 0; i < vn.kids.length; i++) {
    patch(doc, el, el.childNodes[i], old.kids[i], vn.kids[i], fire);
  }
}

const MAX_LOG = 20000;

/**
 * Starts the app in `root`. Returns `{ model, dispatch, swap, stop }`.
 * Runtime failures inside `update`/`view` are reported through `onError` (a `TezError`) and stop the loop.
 *
 * Hot-swap is event sourcing: every UI event is logged as (path in the vnode tree, kind, value). `swap` loads
 * a new program, starts it from its own `init()` and replays the log. The program is pure, so the replay yields
 * exactly the state the user would have reached; events whose target no longer exists end the replay there.
 */
export function mountApp(rt0, root, { onError = (e) => { throw e; } } = {}) {
  if (!isApp(rt0)) throw new TezError("E_INTERNAL", "an app needs `init()`, `update(model, msg)` and `view(model)`");
  const doc = root.ownerDocument;
  let rt = rt0;
  let model;
  let tree;
  let el;
  let dead = false;
  let queued = false;
  let log = [];
  let replaying = false;
  /** running timers, by position in the subscription list */
  let timers = [];
  let subsNow = [];

  const fail = (e) => {
    dead = true;
    onError(e instanceof TezError ? e : new TezError("E_INTERNAL", String(e && e.message ? e.message : e)));
  };

  const pathOf = (node) => {
    const path = [];
    for (let n = node; n && n !== el; n = n.parentNode) {
      path.unshift(Array.prototype.indexOf.call(n.parentNode.childNodes, n));
    }
    return path;
  };

  const step = (vn, kind, value) => {
    const ex = rt.exports;
    const msg = kind === "click" ? vn.click : vn.input !== undefined ? ex.__apply1(vn.input, value) : undefined;
    if (msg !== undefined) model = ex.f_update(model, msg);
    return msg !== undefined;
  };

  const draw = () => {
    const next = readHtml(rt, rt.exports.f_view(model));
    if (!tree) {
      root.textContent = "";
      el = create(doc, next, fire);
      root.appendChild(el);
    } else {
      patch(doc, root, el, tree, next, fire);
    }
    tree = next;
  };

  const render = () => {
    queued = false;
    if (dead) return;
    try {
      draw();
    } catch (e) {
      fail(e);
      return;
    }
    syncTimers();
  };

  const stopTimers = () => {
    for (const t of timers) t?.stop();
    timers = [];
  };

  /** Starts, keeps or stops the subscriptions so that they match `subs(model)`; messages are always the latest. */
  const syncTimers = () => {
    if (dead) return;
    try {
      subsNow = readSubs(rt, model);
    } catch (e) {
      fail(e);
      return;
    }
    while (timers.length > subsNow.length) timers.pop()?.stop();
    const win = doc.defaultView;
    subsNow.forEach((sub, i) => {
      const key = `${sub.kind}:${sub.ms ?? ""}`;
      if (timers[i]?.key === key) return;
      timers[i]?.stop();
      if (sub.kind === "every" || sub.kind === "clock") {
        const id = setInterval(() => tick(i, sub.kind === "clock" ? Math.floor(Date.now() / 1000) : 0), sub.ms);
        timers[i] = { key, stop: () => clearInterval(id) };
      } else if (win) {
        const target = sub.kind === "online" ? win : doc;
        const events = sub.kind === "online" ? ["online", "offline"] : ["visibilitychange"];
        const on = (ev) =>
          tick(i, sub.kind === "online" ? (ev.type === "online" ? 1 : 0) : doc.visibilityState === "visible" ? 1 : 0);
        for (const e of events) target.addEventListener(e, on);
        timers[i] = { key, stop: () => events.forEach((e) => target.removeEventListener(e, on)) };
      } else {
        timers[i] = { key, stop() {} };
      }
    });
  };

  function tick(i, value) {
    if (dead || replaying || !subsNow[i]) return;
    try {
      model = rt.exports.f_update(model, subMessage(rt, subsNow[i], value));
      if (log.length < MAX_LOG) log.push({ kind: "tick", path: [i], value });
    } catch (e) {
      fail(e);
      return;
    }
    schedule();
  }

  function schedule() {
    if (!queued) {
      queued = true;
      queueMicrotask(render);
    }
  }

  function fire(node, kind, value) {
    if (dead || replaying) return;
    try {
      const path = pathOf(node);
      if (!step(node.__tez, kind, value)) return;
      if (log.length < MAX_LOG) log.push({ path, kind, value });
    } catch (e) {
      fail(e);
      return;
    }
    // several events in one tick render once
    schedule();
  }

  model = rt.exports.f_init();
  render();

  return {
    get model() {
      return model;
    },
    /** Sends a message value that came from the running program (not logged: it cannot be replayed). */
    dispatch(msg) {
      if (dead) return;
      try {
        model = rt.exports.f_update(model, msg);
      } catch (e) {
        fail(e);
        return;
      }
      if (!queued) {
        queued = true;
        queueMicrotask(render);
      }
    },
    /** Replaces the program, keeping the user's session. Returns `{ replayed, dropped }`. */
    swap(next) {
      if (dead) return { replayed: 0, dropped: 0 };
      if (!isApp(next)) throw new TezError("E_INTERNAL", "an app needs `init()`, `update(model, msg)` and `view(model)`");
      const oldRt = rt;
      const oldModel = model;
      const oldTree = tree;
      let replayed = 0;
      try {
        rt = next;
        model = next.exports.f_init();
        tree = undefined;
        // replay against virtual trees only; the DOM is patched once at the end
        let cur = readHtml(rt, rt.exports.f_view(model));
        for (const ev of log) {
          if (ev.kind === "tick") {
            const subs = readSubs(rt, model);
            const sub = subs[ev.path[0]];
            if (!sub) break;
            model = rt.exports.f_update(model, subMessage(rt, sub, ev.value));
            cur = readHtml(rt, rt.exports.f_view(model));
            replayed++;
            continue;
          }
          let vn = cur;
          for (const i of ev.path) vn = vn?.kids?.[i];
          if (!vn || vn.text !== undefined || !step(vn, ev.kind, ev.value)) break;
          cur = readHtml(rt, rt.exports.f_view(model));
          replayed++;
        }
        const dropped = log.length - replayed;
        log = log.slice(0, replayed);
        // keep the existing DOM and patch it from the old tree to the new one
        tree = oldTree;
        replaying = true;
        const nextTree = readHtml(rt, rt.exports.f_view(model));
        patch(doc, root, el, tree, nextTree, fire);
        tree = nextTree;
        replaying = false;
        syncTimers();
        return { replayed, dropped };
      } catch (e) {
        // the new program failed while replaying: restore the old one untouched
        replaying = false;
        rt = oldRt;
        model = oldModel;
        tree = oldTree;
        throw e;
      }
    },
    stop() {
      dead = true;
      stopTimers();
      root.textContent = "";
    },
  };
}
