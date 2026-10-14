type Model = { count: number };
type Msg = "inc" | "dec" | "reset";

function init(): Model {
  return { count: 0 };
}

function update(m: Model, msg: Msg): Model {
  switch (msg) {
    case "inc":
      return { ...m, count: m.count + 1 };
    case "dec":
      return m.count > 0 ? { ...m, count: m.count - 1 } : m;
    case "reset":
      return init();
  }
}

function main(): Model {
  const msgs: Msg[] = ["inc", "inc", "inc", "dec", "dec", "dec", "dec", "inc"];
  return msgs.reduce(update, init());
}
