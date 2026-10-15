# Tez — Geliştirme Notları

> Analiz tarihi: 2026-10-15 · Durum: `make check` yeşil · Kaynaklar: `docs/ROADMAP.md`, `spec/`, `bench/results/`
> Bu belge analiz ve özellik önerilerinin özetidir; iş durumunun kaynağı `ROADMAP.md` kalır.

## 1. Proje durumu özeti

- 11 crate'lik Rust derleyici hattı: `tez_syntax` → `tez_types` → (`tez_verify` | `tez_interp` | `tez_wasm`), destek katmanları `tez_diag` (61 kararlı hata kodu), `tez_fmt`, `tez_cli`, `tez_driver`, `tez_web`.
- Faz 1 (çekirdek derleyici), Faz 2 (WasmGC + tarayıcı) ve Faz 4 (doğrulama) ✅. Faz 0/3/5 araçları hazır, model ölçümleri sürüyor.
- Sağlamlık: kanıtlanamayan her şey reddedilir; ~4286 soundness fuzz koşusu, 0 ihlal.
- Tarayıcı: derleyici 257 KB gzip, TEA hot-swap (olay günlüğü replay), Studio + site tamamlanmış.

## 2. Ölçümler (kanıtlar)

### 2.1 Token ekonomisi — `bench/results/tokens-2026-10-15.json` (30 örnek, Gemma 4)

| Metrik | Değer |
|---|---|
| Toplam Tez/TS oranı | 0,99 |
| Kanıt satırları hariç | 0,95 |
| 36/38 anahtar kelime/operatör tek token | yalnız `ensures`, `decreases` iki token |
| Dil kartı | 930 / 1200 token |
| En iyi örnekler | 06-shapes 0,59 · 05-primes 0,75 |
| En kötü | 03-factorial 1,27 (sözleşme satırları; kod-hariç 0,88) |

Not: 14 örneklik ilk ölçümdeki 0,88 oranı, örnek sayısı 30'a çıkınca 0,99'a geriledi —
"Tez her zaman belirgin ölçüde kısa" iddiası doğru değil; kazanç ADT/kayıt ağırlıklı kodda yoğunlaşıyor.

### 2.2 TezBench — `bench/results/tezbench-*-2026-10-15.json` (20 görev, Gemma 4 E4B)

| Koşu | parses | passes |
|---|---|---|
| Tez, kart | 0,75 | 0,30 |
| Tez, kart v2 | 0,85 | 0,65 |
| Tez, kart + tamir döngüsü | 0,90 | 0,55 |
| Phi-4-mini (karşılaştırma) | 0,75 | 0,05 |
| TypeScript referansı | 1,00 | 1,00 |

Okuma: TS %100 — eğitim verisi farkı. Tez'de ana darboğaz söz dizimi değil (parses 0,85),
tür/kanıt gereksinimleri (typed 0,70). LoRA ince ayarı (Faz 5) henüz yok.

## 3. Özellik önerileri (öncelik sırasına göre)

Öncelikler AGENTS.md sırası: 1 hata payı · 2 LLM kavrama · 3 token ekonomisi · hız · WasmGC · tarayıcı.
Her aday kart bütçesi (930/1200) ve gramer büyümesi maliyetiyle değerlendirilmeli.

### A. Hata payını düşürenler (öncelik 1)

- **`filter`/`map` eleman aksiyomu** (ROADMAP ⏸): `all(pred)` ve sıralama korunumu retlerinin ana kaynağı.
  Lambda yakalamalarını aksiyomlamak sağlamsızlık riski taşır — indüksiyon lemması olmadan açma.
- **String interpolation** `` "toplam: {n}" ``: `++` + `toStr` zincirleri olası sık küçük hata kaynağı;
  ~2-3 kart tokeni, tek gramer kuralı. TezBench ret frekansıyla doğrula, sonra ekle.
- **`Result` için `?` / `try`**: zincirli `andThen`/`mapErr` modellerin hata noktası;
  VC üretimine erken-return çevirisi gerekir (maliyetli).
- **`is` desen ifadesi** (`if p is Some(x)`): tek kollu durumlar için `match` yükünü indirir.

### B. LLM kavraması (öncelik 2) — çevre, dil değil

- TezBench'e gerçek dünya görev seti (TEA/UI, dizgi işleme) — 20 görev çoğunlukla algoritmik.
- Her hata kodu için makinede uygulanabilir `suggestedFix` alanı (JSON şema değişimi → `SCHEMA_VERSION` bump).
- `ensures`/`decreases` tek-token kısaltmaları: mevcut programları kırar; yalnız LoRA öncesi
  dil sabitleme noktasında değerlendir.

### C. Token ekonomisi (öncelik 3)

- Prelude genişletme (`zipWith`, `chunks`, `sumBy` …): her metodun kart maliyeti + doğrulayıcı
  aksiyom desteği var. Seçimi TezBench ret frekans analiziyle yap, sezgiyle değil.

### D. Bilinçli olarak eklenmeyecekler

- Döngü/mutasyon/sınıflar — sağlamlık kanıtını ve dar karar uzayını kırar.
- Etkiler (`Cmd`/`Sub`, HTTP, io) — capability modeli olmadan doğrulamayı geçersiz kılar (Faz 7 ⏸ doğru).
- Wasm fast-path (⏸) — `fib(30)` ≈ 0,03 sn; ölçüm darboğaz göstermeden açma.

## 4. Sıradaki somut adım

1. TezBench başarısız görevlerini (kart-v2 koşusunda 7/20) **hata kodu frekansına göre** grupla;
   en sık kod = en yüksek getirili ergonomi düzeltmesi.
2. Model sunucusu kalıcı bağlanınca: `node bench/tezbench/run.mjs --prompts DIR --card` ile
   düzenli ölçüm; sonuçlar `bench/results/` altında `--label` ile sürümlenir.
3. LoRA veri seti büyütme (`data/sample.mjs`) GPU onayı gelince.

## 5. Söz dizimi/gramer sadeleştirme önerileri (2026-10-15)

Kart-v2 TezBench koşusunun ret kırılımı (20 görev): 3 **parse** (03-largest, 05-reverse-text,
17-last-or), 1 **tür** (08-fibonacci), 3 geçersiz çıktı, 13 geçti.
En büyük kayıp söz dizimi üretiminde; öneriler bu kırılıma göre sıralı.

### A. Parse hatası üreten sürtünmeler (en yüksek getiri)

1. **`match` kolları satır tabanlı, virgül yok**: Rust/TS eğitimli modeller `,` ile bitirir
   veya tek satıra çok kol yazar. Kollarda opsiyonel `,` ayırıcı kabul edilmeli; tek gramer
   kuralı, kanıt katmanına dokunmaz. Trailing comma kuralının `args` ve kollara genellemesi.
2. **`if`/`match` koşulunda record literal yasağı**: Kartta takip edilmeyen ince kısıt;
   parantez zorunluluğu (`if (e) { }`, ~2 token) belirsizliği tamamen kaldırır.
3. **`++` dizgi birleştirme**: Modeller refleks olarak `+` yazar. `+`'nın `Str` üzerinde
   aşırı yüklenmesi `catExpr` öncelik seviyesini gramarden siler — hem sadeleşme hem hata
   azaltma; tür çıkarımı sayesinde sağlamsızlık riski yok.

### B. Tür/kanıt hatası üreten sürtünmeler

4. **Yapısal `decreases` çıkarımı**: Desen eşlemeli listeden-kuyruğa özyinelemede küçülen
   argümanı derleyici çıkarabilmeli; `decreases` karttan kısmen düşer (~8-10 token),
   Int-decreases açık yazılmaya devam eder.
5. **`List.map(xs, f)` yasağı**: Kartın açık uyarı ihtiyacının kendisi hata sinyali.
   Uniform-call syntax açmak yöntem çözümlemeyi karmaşıklaştırır; önce ölç.

### C. Grameri küçültenler

6. **`extern`**: Kelime listesinde ama kartta kullanımı görünmüyor — ölü ağırlıksa kaldır.
7. **`with { x: 3 }` ayırıcı tutarlılığı**: `:` hem tür hem fieldInit'te; `=` yalnız `let`'te.
   Yalnız TezBench frekansı gösterirse `with { x = 3 }` değerlendir (kırıcı).
8. **`it` refinement örtük adı**: Modeller bazen lambda yazar; ret mesajı net olduğu sürece beklet.

### D. İdeal kullanım şekli — üretim hattı üçlüsü

1. **Kart + GBNF kısıtlı üretim** (`spec/grammar.gbnf` hazır): Parse hatalarını yapısal
   olarak sıfırlar — bugünkü 3 parse hatasının tamamını ortadan kaldırır; tek başına en
   büyük getiri. llama.cpp/Ollama GBNF destekli; tarayıcı için Faz 6.
2. **Tamir döngüsü açık**: 0,30 → 0,55 geçiş bu döngüden geldi; `suggestedFix` ile ucuzlar.
3. **Kart bütçesini boşaltma**: Sadeleşmelerden kazanılan token, kartta few-shot örnek
   veya prelude metoduna dönüştürülür.

**Önerilen sıra**: (1) `+` Str birleştirme + opsiyonel kol ayırıcıları → (2) yapısal
`decreases` çıkarımı → (3) GBNF kısıtlı üretim. Hepsi sürekli kurala tabi:
`spec/ → tez_fmt → card.md bütçesi → TezBench yeniden ölçümü`.

## 6. Ortam notu (2026-10-15)

`make check` başlangıçta wasm32 hedefinde `E0463: can't find crate for std/core` ile düşüyordu.
Kök neden: Homebrew sade `rust` formülü rustup shim'lerini PATH'te gölgeliyordu.
Düzeltme: `brew unlink rust && brew link --force rustup` (geri alma: `brew link rust`).
Sorun depo kaynaklı değildi; `rust-toolchain.toml` (1.99.0 + wasm32) doğru.
