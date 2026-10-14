type Shape =
  | { kind: "circle"; r: number }
  | { kind: "rect"; w: number; h: number }
  | { kind: "triangle"; b: number; h: number };

function area(s: Shape): number {
  switch (s.kind) {
    case "circle":
      return 3 * s.r * s.r;
    case "rect":
      return s.w * s.h;
    case "triangle":
      return Math.trunc((s.b * s.h) / 2);
  }
}

function describe(s: Shape): string {
  switch (s.kind) {
    case "circle":
      return "circle";
    case "rect":
      return s.w === s.h ? "square" : "rectangle";
    case "triangle":
      return "triangle";
  }
}

function main(): [string, number][] {
  const shapes: Shape[] = [
    { kind: "circle", r: 2 },
    { kind: "rect", w: 3, h: 3 },
    { kind: "rect", w: 2, h: 5 },
    { kind: "triangle", b: 4, h: 5 },
  ];
  return shapes.map((s) => [describe(s), area(s)]);
}
