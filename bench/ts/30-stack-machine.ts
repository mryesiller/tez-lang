type Op = { kind: "push"; n: number } | { kind: "add" } | { kind: "mul" } | { kind: "neg" };

function step(stack: number[], op: Op): number[] {
  switch (op.kind) {
    case "push":
      return [op.n, ...stack];
    case "neg": {
      if (stack.length === 0) return stack;
      const [top, ...rest] = stack;
      return [-top, ...rest];
    }
    case "add": {
      if (stack.length < 2) return stack;
      const [a, b, ...rest] = stack;
      return [a + b, ...rest];
    }
    case "mul": {
      if (stack.length < 2) return stack;
      const [a, b, ...rest] = stack;
      return [a * b, ...rest];
    }
  }
}

function run(ops: Op[]): number[] {
  return ops.reduce(step, [] as number[]);
}

function main(): number[] {
  return run([{ kind: "push", n: 2 }, { kind: "push", n: 3 }, { kind: "add" }, { kind: "push", n: 4 }, { kind: "mul" }, { kind: "neg" }]);
}
