function sum(xs: number[]): number {
  if (xs.length === 0) return 0;
  const [head, ...tail] = xs;
  return head + sum(tail);
}

function reverse(xs: number[]): number[] {
  if (xs.length === 0) return [];
  const [head, ...tail] = xs;
  return [...reverse(tail), head];
}

function main(): [number, number[]] {
  return [sum([1, 2, 3, 4, 5]), reverse([1, 2, 3])];
}
