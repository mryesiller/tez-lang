import glob, sys
sys.path.insert(0, ".")
from earley import Grammar, State
import pathlib
root = pathlib.Path(__file__).resolve().parents[2]
g = Grammar((root / "spec/grammar.gbnf").read_text())
g.rules["root"] = [["ws", "file"]] if False else g.rules["root"]
bad = 0
for p in sorted(glob.glob(str(root / "spec/examples/*.tez"))):
    if p.endswith(".reject.tez"):
        continue
    src = pathlib.Path(p).read_text()
    st = State.initial(g).advance(src)
    ok = st is not None and st.complete()
    if not ok:
        bad += 1
    print("ok  " if ok else "FAIL", pathlib.Path(p).name)
# prefixes are viable, garbage is not
st = State.initial(g)
assert st.advance("fn f(a: Int) -> Int =") is not None
assert st.advance("fn f(a: Int) -> Int = 1 +").complete() is False
assert st.advance("fn )") is None
assert st.advance("fn f() -> Int = 1").complete()
print("all examples derivable" if bad == 0 else f"{bad} failed")
sys.exit(1 if bad else 0)
