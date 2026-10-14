function factorial(n: bigint): bigint {
  return n === 0n ? 1n : n * factorial(n - 1n);
}

function main(): bigint {
  return factorial(30n);
}
