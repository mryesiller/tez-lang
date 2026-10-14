type Account = { owner: string; balance: number };
type Result<T, E> = { ok: true; value: T } | { ok: false; error: E };

function deposit(a: Account, amount: number): Account {
  if (amount <= 0) throw new Error("amount must be positive");
  return { ...a, balance: a.balance + amount };
}

function withdraw(a: Account, amount: number): Result<Account, string> {
  if (amount <= 0) return { ok: false, error: "amount must be positive" };
  if (amount > a.balance) return { ok: false, error: "insufficient funds" };
  return { ok: true, value: { ...a, balance: a.balance - amount } };
}

function main(): [number, string] {
  const acct = deposit({ owner: "ada", balance: 100 }, 50);
  const r = withdraw(acct, 500);
  return r.ok ? [r.value.balance, "ok"] : [acct.balance, r.error];
}
