function fib(n: number): number {
  let a = 0;
  let b = 1;
  for (let i = 0; i < n; i++) {
    [a, b] = [b, a + b];
  }
  return a;
}

function main(): number[] {
  return Array.from({ length: 15 }, (_, i) => i).map(fib);
}
