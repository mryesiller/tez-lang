function toFahrenheit(c: number): number {
  return c * 1.8 + 32.0;
}

function describe(c: number): string {
  if (c < 0.0) return "freezing";
  if (c < 20.0) return "mild";
  return "warm";
}

function main(): [number, string] {
  return [toFahrenheit(100.0), describe(12.5)];
}
