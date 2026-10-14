# Tez — Yol Haritası

> Kaynak: [ARCHITECTURE.md](ARCHITECTURE.md) §12 · Güncelleme: 2026-10-14
> Bu dosya işin **canlı durumudur**. Bir madde bitince `[ ]` → `[x]` yapılır. Faz çıkış kriteri sağlanmadan sonraki faza geçilmez.

Durum: `⬜ başlamadı` · `🟦 sürüyor` · `✅ bitti`

| Faz | Ad | Süre | Durum |
|---|---|---|---|
| A | Altyapı ve ajan ortamı | 1 gün | 🟦 |
| 0 | Dil tasarımı + ölçüm | 2 hafta | ⬜ |
| 1 | Çekirdek derleyici | 3 hafta | ⬜ |
| 2 | WasmGC + tarayıcı | 3 hafta | ⬜ |
| 3 | LLM döngüsü + veri hattı v1 | 2 hafta | ⬜ |
| 4 | Doğrulama (refinement + SMT) | 4 hafta | ⬜ |
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
- [ ] CI iş akışının GitHub'da ilk çalıştırması (yerelde denenemez)
- [x] Uzak depo tanımlı: `origin` = github.com/mryesiller/tez-lang (kullanıcı açacak)
- [ ] İlk commit ve ilk push (kullanıcı onayıyla)
- [x] **Ad kontrolü (2026-10-14):** `tez` crates.io ve npm'de dolu → paket adı **`tez-lang`** (ikisinde de boş; komut `tez` kalır). İç crate'ler `tez_*` (hepsi crates.io'da boş). GitHub deposu: `mryesiller/tez-lang` (crates.io ve npm ile aynı ad)
- [ ] `tez-lang` ve `tez_*` adlarını yayın öncesi rezerve et (ilk sürüm 0.0.x)

**Çıkış kriteri:** `make check` yeşil (✅ yerelde doğrulandı), CI yeşil, ad kontrolü tamam.

## Faz 0 — Dil tasarımı + ölçüm (2 hafta)

- [ ] Referans 3–4B model ailesi(leri) seçimi → [ARCHITECTURE §13](ARCHITECTURE.md)
- [ ] Hedef tokenizer kümesi (≥ 4 aile) ve `bench/tokens` ölçüm aracı
- [ ] Tek-token anahtar kelime/operatör testi (aday listesi → eleme)
- [ ] `spec/grammar.ebnf` (normatif, LL(1))
- [ ] `spec/grammar.gbnf` (EBNF'den üretilir, elle düzenlenmez)
- [ ] 30 örnek program (`spec/examples/`), her biri kanonik biçimde
- [ ] `spec/card.md` ≤ 1.200 token
- [ ] TezBench iskeleti: 20 görev + TypeScript karşılıkları
- [ ] İlk ölçüm: 3–4B model, kartla, kısıtsız → sözdizimi geçerliliği ve token oranı

**Çıkış kriteri:** Mod A'da bir 3–4B modelde ≥ %80 sözdizimi geçerliliği; token oranı ≤ 0,9.

## Faz 1 — Çekirdek derleyici (3 hafta)

- [ ] `tez_syntax`: logos lexer, recursive-descent parser, hata kurtarma, span
- [ ] `tez_resolve`: ad çözümleme
- [ ] `tez_types`: çift yönlü denetim, yerel HM, kapsamlılık
- [ ] `tez_interp`: referans yorumlayıcı (yalnızca test oracle'ı)
- [ ] `tez_diag`: tüm hata kodları + JSON şeması sürüm 1 (sabitlenir)
- [ ] `tez_fmt`: kanonik biçimlendirici (idempotent)
- [ ] `tez_cli`: `check | run | fmt | grammar`
- [ ] Fuzz: parser çökmez, `fmt(fmt(x)) == fmt(x)`
- [ ] §5'teki Faz-1 hata sınıflarının her biri için en az bir **reddedilen** ve bir **kabul edilen** test

**Çıkış kriteri:** Faz-1 garantileri testlerle kanıtlı; 30 örnek program çalışıyor.

## Faz 2 — WasmGC + tarayıcı (3 hafta)

- [ ] `tez_ir`: Core IR (ANF)
- [ ] `tez_wasm`: WasmGC codegen (`wasm-encoder`), `Int` hızlı yol + bigint terfisi
- [ ] Diferansiyel test: `tez_wasm` çıktısı ≡ `tez_interp` çıktısı (tüm örnekler + fuzz)
- [ ] `tez_web`: wasm-bindgen API, derleyici `.wasm` < 1,5 MB (gzip)
- [ ] `runtime/`: TEA döngüsü, Virtual DOM diff, Worker host, fuel, bellek tavanı
- [ ] Hot-swap: durum serileştirme + yapısal göç
- [ ] Studio: tanılar + canlı önizleme
- [ ] Studio paketini küçült: Monaco şu an tüm dil tanımlarıyla 4,2 MB (1,1 MB gzip); çekirdek-only paket veya CodeMirror 6 değerlendir (§7 bütçesi)
- [ ] Artımlı derleme: fonksiyon başına önbellek

**Çıkış kriteri:** Sayaç uygulaması tarayıcıda canlı; ARCHITECTURE §7 bütçeleri (L1 hariç) tutuyor.

## Faz 3 — LLM döngüsü + veri hattı v1 (2 hafta)

- [ ] Studio'da LLM entegrasyonu (sağlayıcıdan bağımsız arayüz)
- [ ] Otomatik düzeltme döngüsü: kod → `tez check --json` → LLM → kod
- [ ] `data/` hattı v1: görev → N aday → reddetmeli örnekleme → kanonikleştirme
- [ ] Düzeltme izi (hatalı kod, JSON hata, düzeltme) üretimi
- [ ] TezBench ilk tam ölçüm (3–4B, 7–8B, öncü model; Tez vs TypeScript)
- [ ] `tez-mcp`: `check`, `fmt`, `run` araçlarını sunan MCP sunucusu

**Çıkış kriteri:** İlk uçtan uca demo; ≤ 3 turda yakınsama ≥ %90 (orta model).

## Faz 4 — Doğrulama (4 hafta)

- [ ] Refinement türleri: `T where P(it)`
- [ ] VC üretimi: sıfıra bölme, sınır, sözleşme, Float, `decreases`
- [ ] `tez_solve` (L1): doğrusal tamsayı aritmetiği + eşitlik/UF, < 1 ms/VC
- [ ] Z3 (L2) tembel yükleme, COOP/COEP
- [ ] Karşı örnek üretimi ve `fixes` önerileri
- [ ] VC önbelleği (normalize hash)
- [ ] **Sağlamlık testleri:** bilerek hatalı 100+ program → hepsi reddedilmeli
- [ ] L1 sonuçlarının Z3 ile çapraz doğrulaması (L1 ≠ Z3 ise hata)

**Çıkış kriteri:** ARCHITECTURE §5'teki tüm sınıflar aktif; sağlamlık testlerinde sıfır yanlış-kabul.

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
