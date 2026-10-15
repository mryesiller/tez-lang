#!/usr/bin/env python3
"""Grammar-constrained greedy decoding with mlx-lm: only tokens that keep the output a viable Tez program are
allowed (spec/grammar.gbnf, via the Earley recognizer). It is the same idea llama.cpp's GBNF sampler implements, so
the grammar can move to any runtime; the point here is to measure what the grammar buys.

  python3 tools/gbnf/constrained.py --model DIR [--adapter DIR] --prompts DIR --out DIR [--no-constraint]
"""
import argparse, pathlib, re, sys
import numpy as np
import mlx.core as mx
from mlx_lm import load, generate
from mlx_lm.sample_utils import make_sampler

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from earley import Grammar, State

ROOT = pathlib.Path(__file__).resolve().parents[2]


def build_grammar():
    g = Grammar((ROOT / "spec/grammar.gbnf").read_text())
    lit = lambda s: [("set", False, frozenset(c)) for c in s]
    # the model answers with one fenced block: ```tez\n <program> ```
    g.rules["tezout"] = [lit("```tez\n") + ["ws", "file"] + lit("```")]
    g._nullable()
    return g


def piece_text(tok, i):
    p = tok.convert_ids_to_tokens(i)
    if p is None:
        return None
    m = re.fullmatch(r"<0x([0-9A-Fa-f]{2})>", p)
    if m:
        return chr(int(m.group(1), 16))
    if p.startswith("<") and p.endswith(">") and len(p) > 2 and not p.startswith("<0x"):
        return None  # special tokens
    return p.replace("▁", " ")


class Constraint:
    def __init__(self, tokenizer, eos_ids):
        self.hf = tokenizer._tokenizer
        self.eos = set(eos_ids)
        self.g = build_grammar()
        self.state = State.initial(self.g, "tezout")
        self.prompt_len = None
        self.text_cache = {}
        self.text = ""

    def text_of(self, i):
        if i not in self.text_cache:
            self.text_cache[i] = piece_text(self.hf, i)
        return self.text_cache[i]

    def __call__(self, tokens, logits):
        ids = tokens.tolist()
        if self.prompt_len is None:
            self.prompt_len = len(ids)
        else:
            t = self.text_of(ids[-1])
            if t is not None:
                nxt = self.state.advance(t)
                if nxt is not None:
                    self.state = nxt
        done = self.state.complete()
        row = np.array(logits[0].astype(mx.float32))
        order = np.argpartition(-row, 256)[:256]
        order = order[np.argsort(-row[order])]
        pick = None
        for i in order:
            i = int(i)
            if i in self.eos:
                if done:
                    pick = i
                    break
                continue
            t = self.text_of(i)
            if t is None or t == "":
                continue
            if done:
                continue  # after the closing fence only the end token is allowed
            if self.state.advance(t) is not None:
                pick = i
                break
        if pick is None:  # nothing in the top 256 fits: scan the whole vocabulary by score
            for i in np.argsort(-row):
                i = int(i)
                t = self.text_of(i)
                if (i in self.eos and done) or (t and not done and self.state.advance(t) is not None):
                    pick = i
                    break
        out = np.full(row.shape, -np.inf, dtype=np.float32)
        out[pick] = 0.0
        return mx.array(out)[None]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--adapter")
    ap.add_argument("--prompts", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--max-tokens", type=int, default=700)
    ap.add_argument("--no-constraint", action="store_true")
    a = ap.parse_args()
    model, tok = load(a.model, adapter_path=a.adapter)
    eos = list(tok.eos_token_ids) if hasattr(tok, "eos_token_ids") else [tok.eos_token_id]
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    for f in sorted(pathlib.Path(a.prompts).glob("*.tez.prompt.txt")):
        prompt = tok.apply_chat_template([{"role": "user", "content": f.read_text()}], add_generation_prompt=True, tokenize=False)
        procs = None if a.no_constraint else [Constraint(tok, eos)]
        text = generate(model, tok, prompt=prompt, max_tokens=a.max_tokens, sampler=make_sampler(temp=0.0), logits_processors=procs)
        blocks = re.findall(r"```[a-zA-Z-]*\n(.*?)```", text, re.S)
        code = "\n".join(blocks) if blocks else text
        (out / f.name.replace(".prompt.txt", "")).write_text(code.strip() + "\n")
        print(f.name, "ok", flush=True)


if __name__ == "__main__":
    main()
