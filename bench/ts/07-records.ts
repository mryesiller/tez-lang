type Point = { x: number; y: number };
type Segment = { from: Point; to: Point };

function translate(p: Point, dx: number, dy: number): Point {
  return { ...p, x: p.x + dx, y: p.y + dy };
}

function manhattan(s: Segment): number {
  return Math.abs(s.to.x - s.from.x) + Math.abs(s.to.y - s.from.y);
}

function main(): [Point, number] {
  const a: Point = { x: 1, y: 2 };
  const b = translate(a, 3, -4);
  return [b, manhattan({ from: a, to: b })];
}
