function quicksort(xs: number[]): number[] {
  if (xs.length === 0) return [];
  const [pivot, ...rest] = xs;
  const smaller = rest.filter((x) => x < pivot);
  const larger = rest.filter((x) => x >= pivot);
  return [...quicksort(smaller), pivot, ...quicksort(larger)];
}

function main(): number[] {
  return quicksort([5, 3, 8, 1, 9, 2, 7]);
}
