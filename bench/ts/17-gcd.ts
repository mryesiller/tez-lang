function gcd(a: number, b: number): number {
  return b === 0 ? a : gcd(b, a % b);
}

function main(): number {
  return gcd(84, 36);
}
