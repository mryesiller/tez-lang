function dot(a: number[], b: number[]): number {
  return a
    .slice(0, Math.min(a.length, b.length))
    .map((x, i) => x * b[i])
    .reduce((s, x) => s + x, 0);
}

function labelled(xs: string[]): string[] {
  return xs.map((x, i) => `${i}: ${x}`);
}

function main(): [number, string[]] {
  return [dot([1, 2, 3], [4, 5, 6]), labelled(["a", "b", "c"])];
}
