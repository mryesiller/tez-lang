function countdown(n: number): number[] {
  return n === 0 ? [0] : [n, ...countdown(n - 1)];
}

function main(): number[] {
  return countdown(5);
}
