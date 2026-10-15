type Item = { title: string; done: boolean };
type Model = { items: Item[]; doneCount: number };

function empty(): Model {
  return { items: [], doneCount: 0 };
}

function add(m: Model, title: string): Model {
  return { ...m, items: [...m.items, { title, done: false }] };
}

function titles(m: Model): string[] {
  return m.items.map((i) => i.title);
}

function pending(m: Model): number {
  return m.items.filter((i) => !i.done).length;
}

function main(): [string[], number] {
  const m = add(add(empty(), "write"), "test");
  return [titles(m), pending(m)];
}
