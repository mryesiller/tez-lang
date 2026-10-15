function encode(xs: number[]): [number, number][] {
  if (xs.length === 0) return [];
  const [x, ...rest] = xs;
  const same = rest.filter((y) => y === x).length;
  return [[x, same + 1], ...encode(rest.filter((y) => y !== x))];
}

function main(): [number, number][] {
  return encode([3, 1, 3, 3, 2, 1]);
}
