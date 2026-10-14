type Expr =
  | { kind: "num"; n: number }
  | { kind: "add"; a: Expr; b: Expr }
  | { kind: "mul"; a: Expr; b: Expr }
  | { kind: "neg"; a: Expr };

function evaluate(e: Expr): number {
  switch (e.kind) {
    case "num":
      return e.n;
    case "add":
      return evaluate(e.a) + evaluate(e.b);
    case "mul":
      return evaluate(e.a) * evaluate(e.b);
    case "neg":
      return -evaluate(e.a);
  }
}

function size(e: Expr): number {
  switch (e.kind) {
    case "num":
      return 1;
    case "add":
    case "mul":
      return 1 + size(e.a) + size(e.b);
    case "neg":
      return 1 + size(e.a);
  }
}

function main(): [number, number] {
  const e: Expr = {
    kind: "add",
    a: { kind: "mul", a: { kind: "num", n: 3 }, b: { kind: "num", n: 4 } },
    b: { kind: "neg", a: { kind: "num", n: 5 } },
  };
  return [evaluate(e), size(e)];
}
