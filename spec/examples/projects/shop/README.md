# shop: a three-file Tez project

`main.tez` uses `cart.tez`, which uses `util/money.tez` (which `main.tez` also uses: a file is loaded once).

```bash
tez check main.tez     # all three files are type-checked and verified together
tez run main.tez       # ("$14.00", 2)
tez test main.tez      # runs the tests of every file
tez build main.tez -o shop.wasm
```
