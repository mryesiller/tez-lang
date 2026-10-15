type Tree = { kind: "leaf" } | { kind: "node"; l: Tree; v: number; r: Tree };

function insert(t: Tree, x: number): Tree {
  if (t.kind === "leaf") return { kind: "node", l: t, v: x, r: t };
  return x < t.v ? { ...t, l: insert(t.l, x) } : { ...t, r: insert(t.r, x) };
}

function depth(t: Tree): number {
  return t.kind === "leaf" ? 0 : 1 + Math.max(depth(t.l), depth(t.r));
}

function size(t: Tree): number {
  return t.kind === "leaf" ? 0 : size(t.l) + 1 + size(t.r);
}

function main(): [number, number] {
  const leaf: Tree = { kind: "leaf" };
  const t = insert(insert(insert(insert(leaf, 5), 2), 8), 1);
  return [size(t), depth(t)];
}
