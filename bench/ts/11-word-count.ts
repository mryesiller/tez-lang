function words(text: string): string[] {
  return text
    .trim()
    .split(" ")
    .filter((w) => w.length > 0);
}

function longest(ws: string[]): string | undefined {
  return [...ws].sort((a, b) => b.length - a.length)[0];
}

function main(): [number, string | undefined] {
  const ws = words("  the quick  brown fox jumps ");
  return [ws.length, longest(ws)];
}
