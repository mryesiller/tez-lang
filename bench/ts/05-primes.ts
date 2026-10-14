function isPrime(n: number): boolean {
  return n >= 2 && Array.from({ length: n - 2 }, (_, i) => i + 2).every((d) => n % d !== 0);
}

function primesBelow(limit: number): number[] {
  return Array.from({ length: limit }, (_, i) => i).filter(isPrime);
}

function main(): number[] {
  return primesBelow(50);
}
