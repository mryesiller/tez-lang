const sum = (xs: number[]): number => xs.reduce((a, b) => a + b, 0);

function rowSums(m: number[][]): number[] {
  return m.map((row) => sum(row));
}

function total(m: number[][]): number {
  return sum(rowSums(m));
}

function flatten(m: number[][]): number[] {
  return m.flatMap((row) => row);
}

function main(): [number[], number, number[]] {
  const m = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
  return [rowSums(m), total(m), flatten(m)];
}
