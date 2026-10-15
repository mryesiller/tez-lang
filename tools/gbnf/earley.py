"""A character-level Earley recognizer for GBNF grammars (the subset spec/grammar.gbnf uses).

It answers the one question a constrained decoder needs: "can this text still be extended to a program?"
(`viable`), and "is this text a complete program?" (`complete`). States are persistent, so a decoder can
test a candidate token without disturbing the text decoded so far.
"""
import re

class Grammar:
    def __init__(self, text):
        self.rules = {}       # name -> list of alternatives; alternative = list of symbols
        self.n_aux = 0
        self._parse(text)
        self._nullable()

    # ---- GBNF parsing -------------------------------------------------------------------------
    def _parse(self, text):
        for line in text.split("\n"):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            name, body = line.split("::=", 1)
            self.src = body.strip()
            self.pos = 0
            alts = self._alts()
            self.rules.setdefault(name.strip(), []).extend(alts)

    def _ws(self):
        while self.pos < len(self.src) and self.src[self.pos] in " \t":
            self.pos += 1

    def _alts(self):
        alts = [self._seq()]
        self._ws()
        while self.pos < len(self.src) and self.src[self.pos] == "|":
            self.pos += 1
            alts.append(self._seq())
            self._ws()
        return alts

    def _seq(self):
        seq = []
        while True:
            self._ws()
            if self.pos >= len(self.src) or self.src[self.pos] in "|)":
                return seq
            seq.append(self._postfix())

    def _aux(self, alts):
        self.n_aux += 1
        name = f"_aux{self.n_aux}"
        self.rules[name] = alts
        return name

    def _postfix(self):
        atom = self._atom()
        while self.pos < len(self.src) and self.src[self.pos] in "?*+":
            op = self.src[self.pos]
            self.pos += 1
            if op == "?":
                atom = self._aux([[atom], []])
            elif op == "*":
                name = f"_aux{self.n_aux + 1}"
                atom = self._aux([[atom, name], []])
            else:
                name = f"_aux{self.n_aux + 1}"
                atom = self._aux([[atom, name], [atom]])
        return atom

    def _atom(self):
        c = self.src[self.pos]
        if c == "(":
            self.pos += 1
            alts = self._alts()
            assert self.src[self.pos] == ")", self.src
            self.pos += 1
            return self._aux(alts)
        if c == '"':
            self.pos += 1
            out = []
            while self.src[self.pos] != '"':
                ch = self.src[self.pos]
                if ch == "\\":
                    self.pos += 1
                    ch = {"n": "\n", "t": "\t", "r": "\r", "\\": "\\", '"': '"'}.get(self.src[self.pos], self.src[self.pos])
                out.append(ch)
                self.pos += 1
            self.pos += 1
            # a literal is a sequence of single-character terminals
            return self._aux([[("set", False, frozenset(ch)) for ch in out]]) if len(out) != 1 else ("set", False, frozenset(out[0]))
        if c == "[":
            self.pos += 1
            neg = self.src[self.pos] == "^"
            if neg:
                self.pos += 1
            chars = set()
            while self.src[self.pos] != "]":
                ch = self.src[self.pos]
                if ch == "\\":
                    self.pos += 1
                    ch = {"n": "\n", "t": "\t", "r": "\r", "\\": "\\", '"': '"', "]": "]", "-": "-"}.get(self.src[self.pos], self.src[self.pos])
                if self.src[self.pos + 1] == "-" and self.src[self.pos + 2] != "]":
                    hi = self.src[self.pos + 2]
                    chars.update(chr(x) for x in range(ord(ch), ord(hi) + 1))
                    self.pos += 3
                else:
                    chars.add(ch)
                    self.pos += 1
            self.pos += 1
            return ("set", neg, frozenset(chars))
        m = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*").match(self.src, self.pos)
        self.pos = m.end()
        return m.group(0)

    # ---- nullable analysis --------------------------------------------------------------------
    def _nullable(self):
        self.nullable = set()
        changed = True
        while changed:
            changed = False
            for name, alts in self.rules.items():
                if name in self.nullable:
                    continue
                for alt in alts:
                    if all(isinstance(s, str) and s in self.nullable for s in alt):
                        self.nullable.add(name)
                        changed = True
                        break


def _match(term, ch):
    _, neg, chars = term
    return (ch not in chars) if neg else (ch in chars)


class State:
    """Earley sets for the text consumed so far. Persistent: `advance` returns a new State."""
    __slots__ = ("g", "sets", "start")

    def __init__(self, g, start, sets):
        self.g, self.start, self.sets = g, start, sets

    @classmethod
    def initial(cls, g, start="root"):
        s0 = set()
        st = cls(g, start, [s0])
        for i in range(len(g.rules[start])):
            st._add(s0, (start, i, 0, 0), None)
        st._close(0)
        return st

    def _add(self, items, item, queue):
        if item not in items:
            items.add(item)
            if queue is not None:
                queue.append(item)

    def _close(self, k):
        g = self.g
        items = self.sets[k]
        queue = list(items)
        while queue:
            name, alt, dot, origin = queue.pop()
            body = g.rules[name][alt]
            if dot < len(body):
                sym = body[dot]
                if isinstance(sym, str):
                    for i in range(len(g.rules[sym])):
                        self._add(items, (sym, i, 0, k), queue)
                    if sym in g.nullable:
                        self._add(items, (name, alt, dot + 1, origin), queue)
            else:
                for (n2, a2, d2, o2) in list(self.sets[origin]):
                    b2 = g.rules[n2][a2]
                    if d2 < len(b2) and b2[d2] == name:
                        self._add(items, (n2, a2, d2 + 1, o2), queue)

    def advance(self, text):
        sets = list(self.sets)
        st = State(self.g, self.start, sets)
        for ch in text:
            k = len(sets) - 1
            nxt = set()
            sets.append(nxt)
            for (name, alt, dot, origin) in sets[k]:
                body = self.g.rules[name][alt]
                if dot < len(body) and not isinstance(body[dot], str) and _match(body[dot], ch):
                    nxt.add((name, alt, dot + 1, origin))
            if not nxt:
                return None
            st._close(k + 1)
        return st

    def complete(self):
        return any(name == self.start and dot == len(self.g.rules[name][alt]) and origin == 0
                   for (name, alt, dot, origin) in self.sets[-1])
