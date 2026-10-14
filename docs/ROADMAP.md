# Tez — Yol Haritası

> Kaynak: [ARCHITECTURE.md](ARCHITECTURE.md) §12 · Güncelleme: 2026-10-14
> Bu dosya işin **canlı durumudur**. Bir madde bitince `[ ]` → `[x]` yapılır. Faz çıkış kriteri sağlanmadan sonraki faza geçilmez.

Durum: `⬜ başlamadı` · `🟦 sürüyor` · `✅ bitti`

| Faz | Ad | Süre | Durum |
|---|---|---|---|
| A | Altyapı ve ajan ortamı | 1 gün | 🟦 |
| 0 | Dil tasarımı + ölçüm | 2 hafta | 🟦 (dil ✅, ölçüm bekliyor) |
| 1 | Çekirdek derleyici | 3 hafta | ✅ |
| 2 | WasmGC + tarayıcı | 3 hafta | 🟦 (derleyici ✅, TEA bekliyor) |
| 3 | LLM döngüsü + veri hattı v1 | 2 hafta | ⬜ |
| 4 | Doğrulama (refinement + SMT) | 4 hafta | 🟦 (çekirdek ✅) |
| 5 | Tez-Coder v0 (ince ayar) | 3 hafta | ⬜ |
| 6 | Tarayıcı içi model | 2 hafta | ⬜ |
| 7 | Etkiler (`Cmd`/`Sub`) | sonra | ⬜ |

---

## Faz A — Altyapı ve ajan ortamı 🟦

- [x] Depo yapısı, git, lisanslar (MIT OR Apache-2.0)
- [x] `.agents/`: ajanlar, beceriler (skills), komutlar, MCP yapılandırması
- [x] Rust workspace + `tez_diag`, `tez_syntax`, `tez_cli` iskeletleri
- [x] `rust-toolchain.toml`, `rustfmt`, `clippy` yapılandırması
- [x] `Makefile` (check, test, wasm, studio)
- [x] Studio iskeleti (Vite + React + Monaco, COOP/COEP başlıkları)
- [x] CI iş akışı (`.github/workflows/ci.yml`)
- [x] Araç zinciri: `rustup` (1.99.0 sabit) + `wasm32-unknown-unknown`, `wasm-tools`, `binaryen`, Z3 — `make doctor` yeşil
- [x] CI ilk çalıştırmaları yeşil; z3 kurulumu eklendi
- [x] Uzak depo tanımlı: `origin` = github.com/mryesiller/tez-lang (kullanıcı açacak)
- [ ] İlk commit ve ilk push (kullanıcı onayıyla)
- [x] **Ad kontrolü (2026-10-14):** `tez` crates.io ve npm'de dolu → paket adı **`tez-lang`** (ikisinde de boş; komut `tez` kalır). İç crate'ler `tez_*` (hepsi crates.io'da boş). GitHub deposu: `mryesiller/tez-lang` (crates.io ve npm ile aynı ad)
- [x] **Ad rezervasyonu (2026-10-14):** `tez-lang` 0.0.1 hem crates.io'da hem npm'de yayında. `tez_*` iç crate'leri Faz 1'de kullanılınca yayınlanır

**Çıkış kriteri:** `make check` yeşil (✅ yerelde doğrulandı), CI yeşil, ad kontrolü tamam.

## Faz 0 — Dil tasarımı + ölçüm (kısmen)

Dil tasarımı tamamlandı ve uygulandı; LLM ölçümleri ertelendi (kullanıcı kararı: önce dil).

- [x] Referans model: **Gemma 4 E4B** (Apache-2.0) → [ARCHITECTURE §9.0](ARCHITECTURE.md)
- [x] `spec/grammar.ebnf` (normatif), `spec/language.md` (anlam), `spec/errors.md` (61 hata kodu, testle senkron)
- [x] Örnek programlar: 14 kabul edilen + 10 reddedilen (`spec/examples/`, testle zorlanır)
- [ ] 30 örneğe tamamla
- [ ] `bench/tokens/tokenizers.toml` ve ölçüm aracı (Gemma 4 `tokenizer.json`, 32 MB indirme — onay gerekir)
- [ ] `spec/grammar.gbnf` (EBNF'den üretim) ve `spec/card.md` ≤ 1.200 token
- [ ] TezBench iskeleti: 20 görev + TypeScript karşılıkları
- [ ] `q4f16` ONNX boyutlarını ölç; küçük deneme LoRA
- [ ] İlk ölçüm: Gemma 4 E4B, kartla → sözdizimi geçerliliği ve token oranı

## Faz 1 — Çekirdek derleyici ✅

- [x] `tez_syntax`: lexer, hata kurtarmalı recursive-descent parser, adlandırma kuralları + otomatik düzeltmeler (46 test)
- [x] `tez_types`: ad çözümleme, çift yönlü çıkarım + birleştirme, yöntem çözümleme, kapsamlılık/erişilebilirlik (Maranget), sonlanma çağrı grafiği, tipli HIR (32 test)
- [x] `tez_interp`: referans yorumlayıcı — sınırsız Int, kapanışlar, kaynak sınırları, isteğe bağlı sözleşme denetimi (18 test)
- [x] `tez_diag`: 61 kararlı hata kodu + JSON şeması v1
- [x] `tez_fmt`: kanonik biçimlendirici, yorumları korur ya da reddeder (14 test, prelude idempotent)
- [x] `tez_cli`: `check | run | test | fmt | grammar` (`--json`, `--no-verify`, `--contracts`)
- [ ] Fuzz: parser çökmez, `fmt(fmt(x)) == fmt(x)` (cargo-fuzz)
- [ ] Diferansiyel test altyapısı (WASM çıktısı ≡ yorumlayıcı) → Faz 2

## Faz 2 — WasmGC + tarayıcı 🟦 (çekirdeği ✅)

- [x] `tez_wasm`: WasmGC kod üretimi (`wasm-encoder`) — tek tip `anyref` gösterimi (küçük Int `i31ref`, büyük Int JS `BigInt`, ADT/demet GC yapıları, kapanışlar `call_ref`)
- [x] `runtime/tez-runtime.mjs`: yerleşik kütüphane + büyük sayı + yapısal eşitlik + değer yazdırıcı (Node ve tarayıcı)
- [x] Diferansiyel test: derlenmiş WASM ≡ yorumlayıcı (7 grup, 120+ parça, 14 örnek, çalışma anı hataları; `wasm-tools validate` ile)
- [x] `tez build`; `tez_web` (`check/format/run/test/build`), tarayıcı derleyicisi **257 KB gzip**
- [x] Studio: Worker'da derleyici, canlı Monaco tanıları, tek tık düzeltme, biçimlendir/çalıştır (derlenmiş WASM)/test, örnek seçici
- [x] Hız: `fib(30)` ≈ 0,03 s (WASM) vs 0,65 s (yerel yorumlayıcı); 10 000+ derinlikli özyineleme
- [ ] `runtime/`: TEA döngüsü (`Html` türü, Virtual DOM diff, olaylar) — **sıradaki büyük iş**
- [ ] Hot-swap: durum serileştirme + yapısal göç
- [ ] Worker + fuel (sonsuz döngü koruması; doğrulanmamış programlar için) ve bellek tavanı
- [ ] Studio paketini küçült (Monaco tüm dil tanımlarıyla 4,2 MB) ve tarayıcıda doğrulama (Z3 WASM ya da `tez_solve`)
- [ ] Artımlı derleme: fonksiyon başına önbellek
- [ ] Kod üretimi: yerel Float/Int fast path, nullary ADT singleton, kuyruk çağrıları (`return_call`)

**Çıkış kriteri:** TEA uygulaması (sayaç) tarayıcıda canlı; ARCHITECTURE §7 bütçeleri.

## Faz 3 — LLM döngüsü + veri hattı v1 (2 hafta)

- [ ] Studio'da LLM entegrasyonu (sağlayıcıdan bağımsız arayüz)
- [ ] Otomatik düzeltme döngüsü: kod → `tez check --json` → LLM → kod
- [ ] `data/` hattı v1: görev → N aday → reddetmeli örnekleme → kanonikleştirme
- [ ] Düzeltme izi (hatalı kod, JSON hata, düzeltme) üretimi
- [ ] TezBench ilk tam ölçüm (3–4B, 7–8B, öncü model; Tez vs TypeScript)
- [ ] `tez-mcp`: `check`, `fmt`, `run` araçlarını sunan MCP sunucusu

**Çıkış kriteri:** İlk uçtan uca demo; ≤ 3 turda yakınsama ≥ %90 (orta model).

## Faz 4 — Doğrulama (büyük bölümü ✅)

- [x] Refinement türleri (`T where P`), sözleşmeler, yapısal + sayısal `decreases`
- [x] VC üretimi (sembolik yürütme): sıfıra bölme, Float alanı, indeks, önkoşul, sonkoşul, refinement, assert, sonlanma
- [x] Z3 (SMT-LIB süreci) ile çözüm; her ihlal kendi koduyla + **sayaç örneği**; kanıtlanamayan reddedilir (`E_UNPROVEN`)
- [x] Çelişkili aksiyom koruması; ilgi alanı kesme; özyinelemeli tanımlar yalnızca sonlanma kanıtlandıktan sonra
- [x] Listeler (uzunluk/eleman/append/take/drop/concat/rev/range), dizgiler, kayan sayılar, ADT'ler, demetler
- [x] Lambda parametreleri liste elemanı olarak bilinir (`map/filter/all/any/count/flatMap/sortBy/fold`)
- [x] Prelude gövdeleri kendi sözleşmelerini sağlıyor (testle)
- [x] 27 doğrulayıcı testi (kabul + her kod için ret + çelişki + soundness tuzakları)
- [ ] `filter/map` sonucunun elemanları hakkında sözleşme (`all(pred)`, eleman eşitliği) — lambda gövdesini kapalı terim olarak aksiyomla
- [ ] `tez_solve` (L1): gömülü hafif çözücü → tarayıcıda doğrulama (Z3 WASM ya da kendi LIA çözücüsü)
- [ ] VC önbelleği (normalize hash)
- [ ] Soundness fuzz: rastgele program üret → doğrulanmışsa yorumlayıcıda ihlal OLMADIĞINI kontrol et

## Faz 5 — Tez-Coder v0 (3 hafta)

- [ ] Eğitim seti v1 (doğrulanmış programlar + düzeltme izleri + mutasyon verisi)
- [ ] Kirlenme kontrolü (TezBench ile çakışma sıfır)
- [ ] 3–4B ince ayar (LoRA), değerlendirme
- [ ] Açık ağırlık + veri seti yayını (lisans kontrolü sonrası)

**Çıkış kriteri:** Mod B pass@1, Mod A'nın belirgin üstü.

## Faz 6 — Tarayıcı içi model (2 hafta)

- [ ] Kuantize Tez-Coder (q4) + WebGPU çalışma zamanı
- [ ] GBNF kısıtlı üretim tarayıcıda
- [ ] İnternetsiz uçtan uca demo

## Faz 7 — Etkiler (sonra)

- [ ] `Cmd`/`Sub`, HTTP, depolama; yetki (capability) modeli

---

## Sürekli kurallar

- Her sözdizimi değişikliği: `spec/` → `tez_fmt` → `card.md` bütçesi → TezBench yeniden ölçümü ([`add-syntax-feature`](../.agents/skills/add-syntax-feature/SKILL.md) becerisi).
- Her yeni hata sınıfı: reddeden **ve** kabul eden test ([`add-diagnostic`](../.agents/skills/add-diagnostic/SKILL.md)).
- Sağlamlık: kanıtlanamayan her şey reddedilir. "Şimdilik kabul et" yoktur.
