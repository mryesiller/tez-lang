function square(n: number): number {
  return n * n;
}

function hypotenuseSquared(a: number, b: number): number {
  return square(a) + square(b);
}

function main(): number {
  return hypotenuseSquared(3, 4);
}
