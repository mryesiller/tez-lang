# Tez — Yol Haritası

> Kaynak: [ARCHITECTURE.md](ARCHITECTURE.md) §12 · Güncelleme: 2026-10-14
> Bu dosya işin **canlı durumudur**. Bir madde bitince `[ ]` → `[x]` yapılır. Faz çıkış kriteri sağlanmadan sonraki faza geçilmez.

Durum: `⬜ başlamadı` · `🟦 sürüyor` · `✅ bitti` · madde işaretleri: `🔒` dış kaynak/onay bekliyor (model, GPU, yayın) · `⏸` bilerek ertelendi (gerekçesiyle)

| Faz | Ad | Süre | Durum |
|---|---|---|---|
| A | Altyapı ve ajan ortamı | 1 gün | 🟦 |
| 0 | Dil tasarımı + ölçüm | 2 hafta | 🟦 (dil ✅, token ölçümü ✅, model ölçümü 🔒) |
| 1 | Çekirdek derleyici | 3 hafta | ✅ |
| 2 | WasmGC + tarayıcı | 3 hafta | ✅ (fast path ⏸) |
| 3 | LLM döngüsü + veri hattı v1 | 2 hafta | 🟦 (araçlar ✅, model ölçümü 🔒) |
| 4 | Doğrulama (refinement + SMT) | 4 hafta | ✅ (filter/map aksiyomu ⏸) |
| 5 | Tez-Coder v0 (ince ayar) | 3 hafta | 🟦 (veri ✅, eğitim 🔒) |
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
- [x] İlk commit ve ilk push (kullanıcı onayıyla)
- [x] **Ad kontrolü (2026-10-14):** `tez` crates.io ve npm'de dolu → paket adı **`tez-lang`** (ikisinde de boş; komut `tez` kalır). İç crate'ler `tez_*` (hepsi crates.io'da boş). GitHub deposu: `mryesiller/tez-lang` (crates.io ve npm ile aynı ad)
- [x] **Ad rezervasyonu (2026-10-14):** `tez-lang` 0.0.1 hem crates.io'da hem npm'de yayında. `tez_*` iç crate'leri Faz 1'de kullanılınca yayınlanır

**Çıkış kriteri:** `make check` yeşil (✅ yerelde doğrulandı), CI yeşil, ad kontrolü tamam.

## Faz 0 — Dil tasarımı + ölçüm (kısmen)

Dil tasarımı tamamlandı ve uygulandı; LLM ölçümleri ertelendi (kullanıcı kararı: önce dil).

- [x] Referans model: **Gemma 4 E4B** (Apache-2.0) → [ARCHITECTURE §9.0](ARCHITECTURE.md)
- [x] `spec/grammar.ebnf` (normatif), `spec/language.md` (anlam), `spec/errors.md` (61 hata kodu, testle senkron)
- [x] Örnek programlar: 14 kabul edilen + 10 reddedilen (`spec/examples/`, testle zorlanır)
- [x] 30 kabul edilen örneğe tamamlandı (+10 reddedilen); her biri için TypeScript karşılığı `bench/ts/` (uygulama örnekleri hariç)
- [x] `bench/tokens`: Gemma 4 tokenizer (indirildi, sürüm sabit) + ölçüm aracı; **36/38 anahtar kelime/operatör tek token**, Tez/TS token oranı **0,88** (28 eşli program; kanıt satırları hariç 0,84; ilk 14 programda 0,82 idi, örnek sayısı artınca yükseldi — kısa programlarda sözleşme satırları ağır basıyor)
- [x] `spec/card.md`: **930 token** (bütçe 1.200); örnekleri doğrulanıyor, metot listesi prelude ile testle senkron
- [x] `spec/grammar.gbnf`: `spec/tools/ebnf2gbnf.py` ile EBNF'den üretilir; `--check` güncelliği ve 30 örneğin türetilebilirliğini doğrular (üst küme: boşluk/yeni satır serbest, anahtar sözcükler tanımlayıcıdan dışlanmaz)
- [x] TezBench iskeleti: `bench/tezbench/` — 20 görev, Tez + TypeScript referansları, dört düzeyli yargıç (ayrıştırır, tür denetler, doğrular, vakaları geçer), `--self-check`, `--prompts`
- [x] `q4f16` ONNX boyutları ölçüldü (yalnız HF üst verisi, ağırlık indirilmedi; `bench/onnx-sizes.mjs`): **metin-only q4f16 = 4,9 GB** (kod çözücü 2,9 + gömme 2,0), q4 5,6 GB, fp16 16,5 GB. Sonuç: Mod C "isteğe bağlı indirme" olmalı (ARCHITECTURE §9 riski doğrulandı); ince ayarlı daha küçük bir model hedefi anlamlı
- [ ] 🔒 Küçük deneme LoRA — GPU ve temel model ağırlığı indirme onayı gerekir
- [ ] 🔒 İlk ölçüm: Gemma 4 E4B, kartla → sözdizimi geçerliliği. Token oranı ölçüldü (0,88); geçerlilik için bir model sunucusu gerekir. Hazır araç: `node bench/tezbench/run.mjs --prompts DIR --card` + `tools/agent/solve.mjs` (OpenAI uyumlu herhangi bir sunucu: Ollama, llama.cpp, vLLM)

## Faz 1 — Çekirdek derleyici ✅

- [x] `tez_syntax`: lexer, hata kurtarmalı recursive-descent parser, adlandırma kuralları + otomatik düzeltmeler (46 test)
- [x] `tez_types`: ad çözümleme, çift yönlü çıkarım + birleştirme, yöntem çözümleme, kapsamlılık/erişilebilirlik (Maranget), sonlanma çağrı grafiği, tipli HIR (32 test)
- [x] `tez_interp`: referans yorumlayıcı — sınırsız Int, kapanışlar, kaynak sınırları, isteğe bağlı sözleşme denetimi (18 test)
- [x] `tez_diag`: 61 kararlı hata kodu + JSON şeması v1
- [x] `tez_fmt`: kanonik biçimlendirici, yorumları korur ya da reddeder (14 test, prelude idempotent)
- [x] `tez_cli`: `check | run | test | fmt | grammar` (`--json`, `--no-verify`, `--contracts`)
- [x] Fuzz: `crates/tez_driver/tests/robust.rs` — tohumlu mutasyon fuzz'ı (örnek başına `TEZ_FUZZ_N`, 4000 ile ~100 bin mutasyon): ön yüz çökmez, `fmt(fmt(x)) == fmt(x)`, biçimli çıktı yeniden ayrışır. İlk koşuda gerçek hata buldu (`(p,)` demet deseni) ve düzeltildi
- [x] Diferansiyel test altyapısı (WASM çıktısı ≡ yorumlayıcı) → Faz 2'de `tez_wasm/tests/differential.rs`

## Faz 2 — WasmGC + tarayıcı ✅

- [x] `tez_wasm`: WasmGC kod üretimi (`wasm-encoder`) — tek tip `anyref` gösterimi (küçük Int `i31ref`, büyük Int JS `BigInt`, ADT/demet GC yapıları, kapanışlar `call_ref`)
- [x] `runtime/tez-runtime.mjs`: yerleşik kütüphane + büyük sayı + yapısal eşitlik + değer yazdırıcı (Node ve tarayıcı)
- [x] Diferansiyel test: derlenmiş WASM ≡ yorumlayıcı (7 grup, 120+ parça, 14 örnek, çalışma anı hataları; `wasm-tools validate` ile)
- [x] `tez build`; `tez_web` (`check/format/run/test/build`), tarayıcı derleyicisi **257 KB gzip**
- [x] Studio: Worker'da derleyici, canlı Monaco tanıları, tek tık düzeltme, biçimlendir/çalıştır (derlenmiş WASM)/test, örnek seçici
- [x] Hız: `fib(30)` ≈ 0,03 s (WASM) vs 0,65 s (yerel yorumlayıcı); 10 000+ derinlikli özyineleme
- [x] `runtime/tez-app.mjs`: TEA döngüsü (`Html[Msg]` türü prelude'da, vnode diff, tıklama/girdi olayları); Studio'da "App" düğmesi; örnekler 15-counter-app, 16-echo-app
- [x] Hot-swap: olay günlüğü (event sourcing) — kod değişince yeni `init()` ile başlayıp kullanıcı olayları yeniden oynatılır; Studio'da ve sitede çalışır
- [x] `site/`: açılış sayfası + dokümantasyon (Tour, referans, hata kataloğu, örnekler, mimari, yol haritası); çalıştırılabilir örnekler tarayıcıda derlenir; GitHub Pages iş akışı (`.github/workflows/pages.yml`)
- [x] Hot-swap: durum serileştirme yerine olay günlüğü yeniden oynatma (yapısal göç gerekmez: program saftır, model her seferinde yeniden hesaplanır)
- [x] Çalıştırma sınırları: derleyici Worker'da; çalıştırma/test 10 sn'yi aşarsa worker öldürülüp yeniden başlatılır (`E_RESOURCE`), liste/üs/büyük sayı tavanları ve bellek hataları `E_RESOURCE`'a eşlenir. Yakıt sayacı yerine duvar saati seçildi (derlenmiş koda sayaç eklemek her çağrıyı yavaşlatır). Sınır: TEA uygulamalarının `update`/`view` çağrıları ana iş parçacığında koşar
- [x] Studio paketi küçültüldü: Monaco çekirdeği + yalnız gerekli katkılar (`studio/src/monaco-lite.js`); ilk yüklenen JS 4,3 MB → 3,45 MB (gzip 1,08 → 0,89 MB), `dist` ≈ 13 MB → 5 MB (ts/css/html çalışanları çıktı)
- [x] Tarayıcıda doğrulama: Z3 WebAssembly + **tekrar oynatma (replay) arka ucu** — doğrulayıcı çözücüyü beklemez; elinde olmayan cevap için `unknown` alır (yalnız reddedebilir), gereken betiği kaydeder; ev sahibi betikleri asenkron çözer; cevaplarla yeniden çalıştırılır, eksik kalmayana kadar. `tez_verify/tests/replay.rs`: 30 programda süreç arka ucuyla birebir aynı sonuç; Studio'da **Verify** düğmesi (36-tree-depth: 211 sorgu, 15 tur, 1,2 sn). Z3 33 MB, yalnız Verify'a basılınca iner; çapraz kaynak izolasyonu gerekir (dev sunucu + Pages için `coi-sw.js`)
- [x] Artımlı derleme gerekmedi (ölçüm): 1.561 satırlık program yerelde 0,08 sn'de tür denetiminden geçiyor; küçük örnekler ölçülemeyecek kadar hızlı. Pahalı olan doğrulamadır ve onu VC önbelleği (betik karması) zaten kapsıyor. Fonksiyon başına tür önbelleği doğruluk riski getirir, kazancı yok — tarayıcıda bir program 5.000 satırı aşıp ölçüm bütçeyi aşarsa yeniden açılır
- [x] Kod üretimi: kuyruk çağrıları (`return_call`/`return_call_ref`; `if`/`match` kollarında da; 5 milyon derinlikli özyineleme sabit yığında çalışır; gereksinim: Chrome 112+/Firefox 121+/Safari 18.2+/Node 20+), nullary ADT singleton (kullanım başına ayırma yok)
- [ ] ⏸ Kod üretimi: yerel Float/Int fast path (kutusuz yerel değişkenler). Ertelendi: `fib(30)` ≈ 0,03 sn bütçenin çok altında; tür-bilgili yerel tipler ayrı bir yeniden düzenleme. Bir kıyas ölçümü darboğaz gösterirse açılır

**Çıkış kriteri:** TEA uygulaması (sayaç) tarayıcıda canlı; ARCHITECTURE §7 bütçeleri.

## Faz 3 — LLM döngüsü + veri hattı v1 🟦 (araçlar ✅, model ölçümü 🔒)

- [x] Studio'da LLM entegrasyonu: "Ask a model" paneli — OpenAI uyumlu herhangi bir sunucu (`tools/agent/providers.mjs`), kart + hata geri besleme, tarayıcıda uçtan uca doğrulandı (sahte sunucuyla)
- [x] Otomatik düzeltme döngüsü: `tools/agent/loop.mjs` (Node + tarayıcı ortak), `solve.mjs` CLI, gerçek derleyiciyle testli
- [x] `data/` hattı v1: `sample.mjs` (görev → N aday → derleyici reddi → kanonikleştirme → tekilleştirme → kirlenme filtresi), `build-v1.mjs`
- [x] Düzeltme izi üretimi: modelsiz mutasyonla (`data/mutate.mjs`, 11 işleç, 17 hata kodu) ve model denemelerinden
- [ ] 🔒 TezBench ilk tam ölçüm (3–4B, 7–8B, öncü model; Tez vs TypeScript) — model sunucusu/API anahtarı gerekir; yargıç ve görevler hazır (`bench/tezbench/`)
- [x] `tez-mcp` (`tools/tez-mcp/`): `tez_check`, `tez_fmt`, `tez_run`, `tez_test`; bağımlılıksız stdio MCP sunucusu, testli

**Çıkış kriteri:** İlk uçtan uca demo; ≤ 3 turda yakınsama ≥ %90 (orta model).

## Faz 4 — Doğrulama ✅

- [x] Refinement türleri (`T where P`), sözleşmeler, yapısal + sayısal `decreases`
- [x] VC üretimi (sembolik yürütme): sıfıra bölme, Float alanı, indeks, önkoşul, sonkoşul, refinement, assert, sonlanma
- [x] Z3 (SMT-LIB süreci) ile çözüm; her ihlal kendi koduyla + **sayaç örneği**; kanıtlanamayan reddedilir (`E_UNPROVEN`)
- [x] Çelişkili aksiyom koruması; ilgi alanı kesme; özyinelemeli tanımlar yalnızca sonlanma kanıtlandıktan sonra
- [x] Listeler (uzunluk/eleman/append/take/drop/concat/rev/range), dizgiler, kayan sayılar, ADT'ler, demetler
- [x] Lambda parametreleri liste elemanı olarak bilinir (`map/filter/all/any/count/flatMap/sortBy/fold`)
- [x] Prelude gövdeleri kendi sözleşmelerini sağlıyor (testle)
- [x] 27 doğrulayıcı testi (kabul + her kod için ret + çelişki + soundness tuzakları)
- [ ] ⏸ `filter/map` sonucunun elemanları hakkında sözleşme (`all(pred)`, eleman eşitliği) — lambda gövdesini kapalı terim olarak aksiyomla. Ertelendi: asıl hedef (quicksort uzunluk korunumu) bölme lemması için indüksiyon gerektirir; yalnız eleman aksiyomu bunu çözmez ve lambda yakalamalarını (env) aksiyomlamak sağlamlık riski taşır. Çalışan çözüm: kullanıcı kendi fonksiyonuna sözleşme yazar (language.md §9)
- [x] `tez_solve` (L1) gerekmedi: tarayıcıda doğrulama Z3 WASM ile çözüldü (yukarıda). Kendi çözücümüz yalnızca 33 MB'ı küçültmek istersek anlamlı olur; sağlamlık riski nedeniyle ertelendi
- [x] VC önbelleği: çözücüye gönderilen betiğin (çözücü sürümü dahil) SHA-256'sı; yalnız `unsat` kaydedilir, yani yanlış bir girdi asla ret → kabul çevirmez; soğuk 0,90 s → sıcak 0,32 s; `TEZ_CACHE_DIR`, `TEZ_NO_CACHE`; `cache_dir` seçeneği
- [x] Soundness fuzz: `tez_verify/tests/soundness.rs` — 300 program, 75 doğrulandı, ~4286 çalıştırma, 0 ihlal

## Faz 5 — Tez-Coder v0 🟦 (veri ✅, eğitim 🔒)

- [x] Eğitim seti v1 (küçük: 26 doğrulanmış program + 108 mutasyon düzeltme izi, `data/v1/`); ölçek model gerektirir (`sample.mjs`)
- [x] Kirlenme kontrolü: `data/contamination.mjs` (eşitlik, 24-token ortak dizi, istem metni); 4 örnek TezBench ile çakıştığı için eğitimden çıkarıldı; testle zorlanır
- [ ] 🔒 3–4B ince ayar (LoRA), değerlendirme — GPU ve temel model gerekir; veri hattı hazır (`data/`)
- [ ] 🔒 Açık ağırlık + veri seti yayını (lisans kontrolü sonrası) — ağırlıklar yok; yayın kullanıcı onayı gerektirir

**Çıkış kriteri:** Mod B pass@1, Mod A'nın belirgin üstü.

## Faz 6 — Tarayıcı içi model (2 hafta)

- [ ] 🔒 Kuantize Tez-Coder (q4) + WebGPU çalışma zamanı — eğitilmiş model gerekir
- [ ] 🔒 GBNF kısıtlı üretim tarayıcıda — `spec/grammar.gbnf` hazır; tarayıcı içi model çalışma zamanı gerekir
- [ ] 🔒 İnternetsiz uçtan uca demo — yukarıdakilere bağlı (Studio zaten çevrimdışı derler, çalıştırır, doğrular)

## Faz 7 — Etkiler (sonra)

- [ ] ⏸ `Cmd`/`Sub`, HTTP, depolama; yetki (capability) modeli — kullanıcı kararı: ilk aşamada yok

---

## Sürekli kurallar

- Her sözdizimi değişikliği: `spec/` → `tez_fmt` → `card.md` bütçesi → TezBench yeniden ölçümü ([`add-syntax-feature`](../.agents/skills/add-syntax-feature/SKILL.md) becerisi).
- Her yeni hata sınıfı: reddeden **ve** kabul eden test ([`add-diagnostic`](../.agents/skills/add-diagnostic/SKILL.md)).
- Sağlamlık: kanıtlanamayan her şey reddedilir. "Şimdilik kabul et" yoktur.
