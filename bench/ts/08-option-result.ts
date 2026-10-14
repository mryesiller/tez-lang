type Result<T, E> = { ok: true; value: T } | { ok: false; error: E };

function parseAge(s: string): Result<number, string> {
  const n = Number.parseInt(s, 10);
  if (Number.isNaN(n)) return { ok: false, error: "not a number: " + s };
  if (n < 0) return { ok: false, error: "negative age" };
  return { ok: true, value: n };
}

function describe(r: Result<number, string>): string {
  return r.ok ? "age " + String(r.value) : "error: " + r.error;
}

function main(): string[] {
  return ["42", "abc", "-5"].map((s) => describe(parseAge(s)));
}
