function lookup(xs: number[], i: number): number | undefined {
  return i >= 0 && i < xs.length ? xs[i] : undefined;
}

function firstOr(xs: number[], fallback: number): number {
  return lookup(xs, 0) ?? fallback;
}

function main(): [number | undefined, number | undefined, number] {
  return [lookup([10, 20, 30], 1), lookup([10, 20, 30], 7), firstOr([], 99)];
}
