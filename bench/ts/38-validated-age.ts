function category(a: number): string {
  if (a < 0 || a > 150) throw new Error("age out of range");
  return a < 18 ? "minor" : a < 65 ? "adult" : "senior";
}

function describe(s: string): string {
  const n = Number.parseInt(s, 10);
  if (Number.isNaN(n)) return "not a number";
  return n >= 0 && n <= 150 ? category(n) : "out of range";
}

function main(): string[] {
  return ["12", "40", "70", "abc", "200"].map(describe);
}
