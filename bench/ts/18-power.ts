function power(base: bigint, exp: number): bigint {
  return exp === 0 ? 1n : base * power(base, exp - 1);
}

function main(): [bigint, bigint] {
  return [power(2n, 10), power(3n, 40)];
}
