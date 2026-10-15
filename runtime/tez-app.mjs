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

/** The active subscriptions of a model: `[{ms, msg}]` from the optional `subs(model)` function. */
export function readSubs(rt, model) {
  if (!rt.meta.fns.subs || rt.meta.fns.subs.params !== 1) return [];
  const ex = rt.exports;
  return ex.f_subs(model).map((s) => ({ ms: Math.max(50, Number(ex.__get0(s))), msg: ex.__get1(s) }));
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

function create(doc, vn, fire) {
  if (vn.text !== undefined) return doc.createTextNode(vn.text);
  const el = doc.createElement(vn.tag);
  setProps(el, {}, vn);
  bind(el, vn, fire);
  for (const k of vn.kids) el.appendChild(create(doc, k, fire));
  return el;
}

function setProps(el, old, vn) {
  for (const k of Object.keys(old.props ?? {})) {
    if (!(k in vn.props)) el.removeAttribute(k);
  }
  for (const [k, v] of Object.entries(vn.props)) {
    if (old.props?.[k] === v) continue;
    if (k === "value" && "value" in el) {
      if (el.value !== v) el.value = v;
    } else {
      el.setAttribute(k, v);
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
  const same = old.text !== undefined ? vn.text !== undefined : vn.text === undefined && old.tag === vn.tag;
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
    for (const t of timers) clearInterval(t.id);
    timers = [];
  };

  /** Starts, keeps or stops timers so that they match `subs(model)`; messages are always the latest ones. */
  const syncTimers = () => {
    if (dead) return;
    try {
      subsNow = readSubs(rt, model);
    } catch (e) {
      fail(e);
      return;
    }
    while (timers.length > subsNow.length) clearInterval(timers.pop().id);
    subsNow.forEach((sub, i) => {
      if (timers[i] && timers[i].ms === sub.ms) return;
      if (timers[i]) clearInterval(timers[i].id);
      timers[i] = { ms: sub.ms, id: setInterval(() => tick(i), sub.ms) };
    });
  };

  function tick(i) {
    if (dead || replaying || !subsNow[i]) return;
    try {
      model = rt.exports.f_update(model, subsNow[i].msg);
      if (log.length < MAX_LOG) log.push({ kind: "tick", path: [i] });
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
            model = rt.exports.f_update(model, sub.msg);
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
