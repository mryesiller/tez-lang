function fizzbuzz(n: number): string {
  if (n % 15 === 0) return "FizzBuzz";
  if (n % 3 === 0) return "Fizz";
  if (n % 5 === 0) return "Buzz";
  return String(n);
}

function main(): string[] {
  return Array.from({ length: 15 }, (_, i) => i + 1).map(fizzbuzz);
}
