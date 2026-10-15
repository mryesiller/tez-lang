function normalize(s: string): string {
  return s.toLowerCase().trim();
}

function isPalindrome(s: string): boolean {
  const cs = [...normalize(s)];
  return cs.join("") === [...cs].reverse().join("");
}

function main(): [boolean, boolean] {
  return [isPalindrome("Level"), isPalindrome("Tez")];
}
