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

function create(doc, vn, send) {
  if (vn.text !== undefined) return doc.createTextNode(vn.text);
  const el = doc.createElement(vn.tag);
  setProps(el, {}, vn);
  bind(el, vn, send);
  for (const k of vn.kids) el.appendChild(create(doc, k, send));
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
function bind(el, vn, send) {
  el.__tez = vn;
  if (el.__bound) return;
  el.__bound = true;
  el.addEventListener("click", () => {
    const cur = el.__tez;
    if (cur.click !== undefined) send(cur.click);
  });
  el.addEventListener("input", () => {
    const cur = el.__tez;
    if (cur.input !== undefined) send(cur.input, el.value);
  });
}

function patch(doc, parent, el, old, vn, send) {
  if (!el) {
    parent.appendChild(create(doc, vn, send));
    return;
  }
  const same = old.text !== undefined ? vn.text !== undefined : vn.text === undefined && old.tag === vn.tag;
  if (!same) {
    parent.replaceChild(create(doc, vn, send), el);
    return;
  }
  if (vn.text !== undefined) {
    if (old.text !== vn.text) el.nodeValue = vn.text;
    return;
  }
  setProps(el, old, vn);
  bind(el, vn, send);
  const n = Math.max(old.kids.length, vn.kids.length);
  for (let i = vn.kids.length; i < old.kids.length; i++) el.removeChild(el.lastChild);
  for (let i = 0; i < Math.min(n, vn.kids.length); i++) {
    patch(doc, el, el.childNodes[i], old.kids[i], vn.kids[i], send);
  }
}

/**
 * Starts the app in `root`. Returns `{ model, dispatch, stop }`.
 * Runtime failures inside `update`/`view` are reported through `onError` (a `TezError`) and stop the loop.
 */
export function mountApp(rt, root, { onError = (e) => { throw e; } } = {}) {
  if (!isApp(rt)) throw new TezError("E_INTERNAL", "an app needs `init()`, `update(model, msg)` and `view(model)`");
  const ex = rt.exports;
  const doc = root.ownerDocument;
  let model = ex.f_init();
  let tree;
  let el;
  let dead = false;
  let queued = false;

  const fail = (e) => {
    dead = true;
    onError(e instanceof TezError ? e : new TezError("E_INTERNAL", String(e && e.message ? e.message : e)));
  };

  const render = () => {
    queued = false;
    if (dead) return;
    try {
      const next = readHtml(rt, ex.f_view(model));
      if (!tree) {
        root.textContent = "";
        el = create(doc, next, send);
        root.appendChild(el);
      } else {
        patch(doc, root, el, tree, next, send);
      }
      tree = next;
    } catch (e) {
      fail(e);
    }
  };

  const send = (msgOrFn, arg) => {
    if (dead) return;
    try {
      const msg = arg === undefined ? msgOrFn : ex.__apply1(msgOrFn, arg);
      model = ex.f_update(model, msg);
    } catch (e) {
      fail(e);
      return;
    }
    // several events in one tick render once
    if (!queued) {
      queued = true;
      queueMicrotask(render);
    }
  };

  render();
  return {
    get model() {
      return model;
    },
    dispatch: (msg) => send(msg),
    stop() {
      dead = true;
      root.textContent = "";
    },
  };
}
