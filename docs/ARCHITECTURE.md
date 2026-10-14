# Tez — Mimari Spesifikasyon v0.3

> Durum: Taslak · Tarih: 2026-10-14 · Lisans: açık kaynak (MIT OR Apache-2.0)
>
> **Tez** (Türkçe: *hızlı*) — 3–4B parametreli modellerin bile yüksek doğrulukla yazabildiği, derlenen her programın tanımlı hata sınıflarından arınmış olduğu kanıtlanan, WebAssembly'ye derlenen ve tamamen tarayıcıda geliştirilebilen bir programlama dili.

## 0. Sürüm Geçmişi

**v0.5 (uygulama)** — v0.4 tasarımı gerçekleştirildi; sapmalar:
- Doğrulama: ilk sürümde **Z3 süreci** (SMT-LIB); gömülü `tez_solve` (L1) ve tarayıcıda Z3 WASM ertelendi. Tarayıcıdaki `check` doğrulamayı **çalıştırmaz** ve bunu `verified: false` ile açıkça bildirir.
- Hata kodu sayısı 61; `decreases` artık `Int` ya da veri değeri (yapısal ölçü); özyinelemeli tanımlar yalnızca sonlanma kanıtlandıktan sonra çözücüye verilir.
- Kod üretimi: **tek tip `anyref` gösterimi** (spec için bkz. `crates/tez_wasm`), kütüphane JS çalışma zamanında (`runtime/`). Yorumlayıcı referans/oracle olarak kalır; her program için derlenmiş WASM ≡ yorumlayıcı diferansiyel testle doğrulanır.
- Sözdizimi: boru (`|>`) yok, tek yol metot sözdizimi (`xs.map(f)`); `List.map(xs, f)` hata ve otomatik düzeltme üretir.
- Soundness: aksiyom tutarlılık denetimi, ilgi alanı kesme, rastgele program fuzz'ı (kabul edilen program çalışırken ihlal üretmez).

**v0.4**
- **Referans model: Gemma 4 E4B** (Apache-2.0). Tokenizer, kart ve ölçümler buna göre ayarlanır (§9.0).
- Tek-token kuralı: **birincil tokenizer'da (Gemma 4) zorunlu**, ikincil tokenizer'larda yalnızca raporlanır.
- Mod C için yol netleşti: `onnx-community/gemma-4-E4B-it-ONNX` + Transformers.js (`q4f16`, WebGPU).

**v0.3**
- Ad: AILang/Axiom → **Tez** (`.tez`, CLI `tez`). Sunholo'nun AILang projesiyle ad çakışması.
- Hedef model boyutu: **3–4B**. Dil kartı bütçesi 2.000 → **1.200 token**.
- Yeni §9: **Eğitim seti ve model stratejisi** (derleyici-doğrulamalı sentetik veri, ince ayar).
- Bytecode VM kaldırıldı → **doğrudan WasmGC codegen** (Faz 2). Referans yorumlayıcı yalnızca test için.
- Doğrulama iki katmanlı: **gömülü hafif çözücü (Rust, WASM içinde)** + gerektiğinde tembel yüklenen Z3.
- `Int`: keyfi hassasiyet, **i64 hızlı yol + taşmada bigint'e terfi**.
- Yeni §7: **Hız bütçeleri** (derleme/doğrulama gecikme hedefleri).
- Yeni §10: Konumlanma (Sunholo AILang, MoonBit vb.).

**v0.2:** Bağımlı türler → refinement + SMT; tanıdık kanonik sözdizimi; TEA runtime; Worker + fuel; logos + elle yazılmış parser.

## 1. Öncelikler

Tasarım çatışmalarında bu sıra hakemdir:

1. **Minimum hata** — derlenen program tanımlı hata sınıflarını üretemez (§5); dil, yanlış yazılması zor olacak şekilde tasarlanır.
2. **LLM anlaşılırlığı** — 3–4B modeller dil kartı + kısıtlı üretimle, ince ayarlı modeller kartsız yüksek doğrulukla yazabilmeli.
3. **Token tasarrufu** — aynı programı TypeScript'ten daha az token'la ifade etmek; birden çok tokenizer'da ölçülür.
4. **Hız** — derleme, doğrulama ve çalışma; tümü tarayıcıda (§7).
5. **WebAssembly** — tek hedef WasmGC; native/JS hedefi yok.
6. **Tamamen tarayıcıda geliştirme** — kurulum gerektirmeyen editör + derleyici + doğrulayıcı + runtime (+ isteğe bağlı tarayıcı içi LLM).

**Kapsam dışı (ilk aşama):** dış yan etkiler (HTTP, depolama, zamanlayıcı), paket yöneticisi, çoklu dosya modülleri.

## 2. Dil Tasarım İlkeleri

1. **Tek kanonik yol.** Her kavramın tek yazımı vardır; `tez fmt` bunu zorlar. Varyasyon yok → küçük modelin olasılık kütlesi tek yola yoğunlaşır.
2. **Tanıdıklık.** Her yapı eğitim verisinde bol bulunan bir dilden (TypeScript, Rust, Kotlin, Elm) gelir. Yeni sembol icat edilmez.
3. **Yerel akıl yürütme.** Üst düzey fonksiyonlarda tür imzası zorunlu; gölgeleme, aşırı yükleme, örtük dönüşüm yok. Bir satırı anlamak uzak bağlam gerektirmez — küçük modellerin kısa dikkat menzili için kritik.
4. **Yanlış yazımı imkânsız kılan sözdizimi.** `null`, istisna, değişken atama, `for/while`, sınıf, kalıtım, makro, varsayılan parametre yok. `&&` ile `||` parantezsiz karıştırılamaz.
5. **LL(1), bağlamdan bağımsız gramer.** GBNF olarak yayımlanır → kısıtlı üretimle sözdizimi hatası sıfır.
6. **Tek-token anahtar kelimeler.** Her anahtar kelime ve operatör, **birincil tokenizer'da (Gemma 4, §9.4)** tek token olmalı; olmayan aday reddedilir. İkincil tokenizer'larda sonuç raporlanır ama engelleyici değildir.
7. **Küçük yüzey alanı.** Dil kartı ≤ 1.200 token. Eklenen her özellik bu bütçeden pay alır; bütçeyi aşan özellik kabul edilmez.

## 3. Sözdizimi Taslağı (v0)

```tez
// Tek yorum biçimi

// --- Türler ---
type Pos = Int where it > 0
type Point = { x: Int, y: Int }
type Shape = Circle(r: Pos) | Rect(w: Pos, h: Pos)

// --- Fonksiyon gövdesi her zaman `= ifade` ---
fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Rect(w, h) => w * h
}

fn div(a: Int, b: Int where it != 0) -> Int = a / b

// --- Sözleşmeler ---
fn avg(xs: List[Int]) -> Int
  requires len(xs) > 0
  = sum(xs) / len(xs)

fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  ensures lo <= result && result <= hi
  = if x < lo { lo } else if x > hi { hi } else { x }

// --- Özyineleme: sonlanma kanıtı zorunlu ---
fn fact(n: Int where it >= 0) -> Int
  decreases n
  = if n == 0 { 1 } else { n * fact(n - 1) }

// --- Blok, let, pipe, lambda ---
fn score(xs: List[Int]) -> Int = {
  let evens = xs |> List.filter(x => x % 2 == 0)
  List.fold(evens, 0, (acc, x) => acc + x) * 2
}

// --- Değişmez kayıt güncelleme ---
fn moveRight(p: Point) -> Point = { p with x: p.x + 1 }
```

| Karar | Seçim | Neden |
|---|---|---|
| Blok | `{ }` | Girinti duyarlılığı küçük modellerde ve kopyala/yapıştırda kırılgan; parantez hatası yerel tespit edilir |
| Satır sonu | Noktalı virgül yok | Token tasarrufu |
| Generic | `List[Int]` | `<>` karşılaştırma ile belirsiz |
| Lambda | `x => e`, `(a, b) => e` | TypeScript'te en yaygın |
| Refinement | `T where it > 0` | Okunur İngilizce; `it` Kotlin'den |
| Sözleşme | `requires` / `ensures` / `decreases` | Dafny standardı |
| Gövde | Her zaman `= ifade` | Tek kanonik yol |
| Match kolu | Satır başına bir kol, virgül yok | Daha az token ve hata |
| Adlandırma | değer `camelCase`, tür `PascalCase` | Tek kural; parser ayrımı büyük harften yapar |
| Erişim | `List.map` | Aşırı yükleme ve ad çakışması yok |

> Bu tablo Faz 0'daki tokenizer ve küçük-model ölçümleriyle (§9) kesinleşir. Ölçüm bir kararı kötü gösterirse karar değişir.

## 4. Tür Sistemi

- **Temel türler:** `Int` (keyfi hassasiyet), `Float` (f64), `Bool`, `Str`, `Unit`.
- **Bileşik:** kayıt, toplam türleri (ADT), `List[T]`, `Map[K, V]`, `Option[T]`, `Result[T, E]`.
- **Parametrik polimorfizm** (generic); trait/typeclass **yok** (v1'de değerlendirilir — küçük modeller için karmaşıklık maliyeti yüksek).
- **Çift yönlü denetim:** üst düzey imzalar zorunlu; fonksiyon içi HM yerel çıkarım.
- **Refinement türleri:** `T where P(it)`, burada P karar verilebilir parçada (§5.1) bir yüklem.
- **`Int` gösterimi:** WasmGC'de `i31ref`/i64 hızlı yol; taşma anında bigint nesnesine terfi. Taşma hiçbir zaman hata değildir ve kanıt yükü getirmez.

## 5. Garanti Modeli

> **Derleme başarılıysa program aşağıdaki hata sınıflarının hiçbirini çalışma anında üretemez ve yazılmış tüm sözleşmelere uyar. Kanıtlanamayan her durum reddedilir (sound).**

Hiçbir sistem programın *istenen şeyi* yaptığını kanıtlayamaz; mantık hataları yalnızca `ensures` ile ifade edildikleri ölçüde yakalanır. Bu nedenle eğitim seti, her fonksiyona sözleşme yazmayı öğretir (§9).

| Hata sınıfı | Mekanizma | Faz |
|---|---|---|
| Null / tanımsız | `null` yok; `Option[T]` | 1 |
| Tür uyuşmazlığı | Çift yönlü denetim | 1 |
| Eksik match | Kapsamlılık denetimi | 1 |
| Yakalanmamış istisna | İstisna yok; `Result[T, E]` | 1 |
| Yan etki / paylaşılan durum | Saf dil, değişmez veri | 1 |
| Tanımsız isim | Ad çözümleme | 1 |
| Tamsayı taşması | Keyfi hassasiyetli `Int` | 1 |
| Sıfıra bölme | `b != 0` VC | 4 |
| Sınır dışı erişim | `0 <= i < len(xs)` VC | 4 |
| Sözleşme ihlali | `requires`/`ensures`/`where` VC | 4 |
| Sonlanmama | Döngü yok; `decreases` VC; yalnızca sınırlı `List.fold/map/range` | 4 |
| Float NaN/∞ | `/` için `b != 0.0`, `sqrt` için `x >= 0.0` VC | 4 |
| Tarayıcı donması | Sonlanma kanıtı + Worker + fuel (savunma derinliği) | 2 |
| Sandbox kaçışı | WASM yetki modeli: yalnızca runtime'ın verdiği import'lar | 2 |
| Bellek tükenmesi | Runtime tavanı → kontrollü `E_RESOURCE` (statik kanıtlanamayan tek sınıf) | 2 |

### 5.1 İki katmanlı doğrulama (hız için)

| Katman | Ne | Nerede | Kapsam |
|---|---|---|---|
| **L1 — `tez_solve`** | Rust'ta yazılmış hafif karar prosedürü: doğrusal tamsayı aritmetiği (simplex + branch-and-bound / Fourier–Motzkin), eşitlik + yorumlanmamış fonksiyonlar (congruence closure), `len` ölçümleri | Derleyiciyle aynı `.wasm` içinde | Pratikte VC'lerin büyük çoğunluğu (sıfıra bölme, sınırlar, basit sözleşmeler); hedef < 1 ms/VC |
| **L2 — Z3** | `z3-solver` (resmi WASM derlemesi) | Ayrı Worker; **yalnızca L1 `unknown` dediğinde** tembel yüklenir | Doğrusal olmayan aritmetik, karmaşık niceleyiciler |

- L2 yüklenemezse veya zaman aşımına uğrarsa → `E_UNPROVEN` + öneri. Sağlamlık hiçbir zaman hız için feda edilmez.
- Z3 tarayıcıda `SharedArrayBuffer` gerektirir → sayfa COOP/COEP başlıklarıyla sunulur.
- Her VC, normalize edilmiş biçiminin hash'iyle önbelleğe alınır; değişmeyen fonksiyonlar yeniden kanıtlanmaz.

## 6. Derleyici Mimarisi

```
kaynak (.tez)
  ▼ tez_syntax   logos lexer → recursive-descent parser (hata kurtarmalı) → AST + span
  ▼ tez_resolve  ad çözümleme
  ▼ tez_types    çift yönlü denetim + yerel HM + kapsamlılık + refinement taşıma
  ▼ tez_verify   VC üretimi → L1 tez_solve → (gerekirse) L2 Z3
  ▼ tez_ir       Core IR (ANF), refinement'lar silinir
  ▼ tez_wasm     WasmGC codegen (wasm-encoder)  ──►  .wasm modülü
  ╰ tez_interp   referans yorumlayıcı (yalnızca test: codegen ile diferansiyel test)
Her aşama ─► tez_diag  tek tip Diagnostic → JSON
```

**Artımlılık:** Derleme birimi fonksiyondur. Her fonksiyonun (imza hash'i, gövde hash'i) çifti tür denetimi, VC ve codegen sonuçlarını önbelleğe alır. Editörde bir tuş vuruşu yalnızca değişen fonksiyonu ve imzası değiştiyse çağıranlarını yeniden işler.

**Neden VM değil WasmGC:** WasmGC tüm büyük tarayıcılarda destekleniyor; çöp toplayıcıyı tarayıcı sağlar → küçük çıktı, kendi GC'mizi yazmayız, yerel hıza yakın çalışma. Küçük modüllerin `WebAssembly.compile` süresi milisaniye düzeyinde olduğu için ayrı bir VM'e gerek kalmaz.

### Repo yapısı

```
tez/
├── .agents/         # ortak ajanlar, beceriler, komutlar, MCP (tek doğruluk kaynağı)
├── docs/            # ARCHITECTURE.md, ROADMAP.md
├── crates/
│   ├── tez_syntax/  tez_resolve/  tez_types/  tez_verify/  tez_solve/
│   ├── tez_ir/      tez_wasm/     tez_interp/  tez_diag/   tez_fmt/
│   ├── tez_cli/     # `tez check | run | build | fmt | grammar`
│   └── tez_web/     # wasm-bindgen API
├── runtime/         # TS: TEA döngüsü, Virtual DOM diff, Worker host, hot-swap
├── studio/          # React + Monaco: tarayıcı IDE'si
├── spec/
│   ├── grammar.ebnf      # normatif
│   ├── grammar.gbnf      # kısıtlı üretim
│   └── card.md           # ≤ 1.200 token dil kartı
├── data/            # eğitim seti üretim hattı (§9)
└── bench/           # TezBench
```

## 7. Hız Bütçeleri

Tarayıcıda, orta seviye bir dizüstünde ölçülür. CI'da regresyon testi olarak izlenir.

| İşlem | Hedef |
|---|---|
| Derleyici `.wasm` boyutu (gzip, Z3 hariç) | < 1,5 MB |
| Studio ilk açılış (Z3 hariç) | < 1,5 s |
| Tuş vuruşu → tanılar (1.000 satır, artımlı, L1) | < 50 ms |
| Tam derleme + doğrulama (1.000 satır, L1, soğuk) | < 300 ms |
| Kaydet → hot-swap → yeni ekran | < 100 ms |
| L2 (Z3) ilk yükleme | arka planda; editörü bloklamaz |

## 8. Runtime: Elm Architecture + Sandbox

```tez
type Model = { count: Int where it >= 0 }
type Msg = Inc | Dec

fn init() -> Model = { count: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Inc => { m with count: m.count + 1 }
  Dec => if m.count > 0 { { m with count: m.count - 1 } } else { m }
}

fn view(m: Model) -> Html[Msg] =
  row([ button(Dec, "-"), text(m.count), button(Inc, "+") ])
```

`else { m }` silinirse doğrulayıcı `count >= 0` ihlalini `m.count = 0, msg = Dec` karşı örneğiyle reddeder.

- **Yürütme:** Modül bir Web Worker'da çalışır; her `update`/`view` çağrısı fuel (codegen'in döngü/özyineleme noktalarına eklediği sayaç) ve bellek tavanıyla sınırlı.
- **DOM:** Worker Virtual DOM ağacını ana thread'e gönderir; ana thread diff alıp uygular; olaylar `Msg` olarak döner.
- **Hot-swap:** `Model` kanonik bir değer kodlamasıyla serileştirilir → yeni modül örneklenir → durum geri yüklenir. `Model` türü değiştiyse yapısal göç (aynı ad + tür alanlar taşınır, yeniler varsayılan alır); mümkün değilse `init()` ve kullanıcıya bildirim.
- **Yan etkiler (Faz 7+):** TEA `Cmd`/`Sub`.

## 9. LLM Stratejisi: Eğitim Seti, Model, Ölçüm

### 9.0 Referans model: Gemma 4 E4B

| Özellik | Değer (kaynak: model kartı) |
|---|---|
| Lisans | Apache-2.0 → ince ayarlı ağırlıkları ve veri setini açık yayınlayabiliriz |
| Boyut | 4,5B etkin parametre, gömülemelerle 8B toplam (Per-Layer Embeddings) |
| Sözlük / bağlam | 262K token / 128K |
| Mimari | 42 katman, 512 token kayan pencere + global dikkat karışımı; görüntü ve ses kodlayıcıları var |
| Kod başarımı (talimatlı) | LiveCodeBench v6: %52,0 · Codeforces ELO: 940 |
| Önerilen örnekleme | sıcaklık 1,0 · top_p 0,95 · top_k 64 |
| Düşünme modu | `enable_thinking` ile açılır/kapanır |
| Tarayıcı | `onnx-community/gemma-4-E4B-it-ONNX`, Transformers.js, `q4f16`, WebGPU |

Sonuçları ve tasarım etkileri:
- **262K sözlük**, yaygın kelimelerin tek token olma ihtimalini artırır; yine de her anahtar kelime ölçülür (§2, ilke 6).
- **Kart bütçesi** (1.200 token) bağlam sınırından değil, küçük modelde dikkat kalitesinden ve tarayıcıda ilk dolum (prefill) gecikmesinden gelir; 128K bağlam bunu değiştirmez.
- **Benchmark protokolü:** her ölçüm hem *greedy* hem önerilen örneklemeyle, düşünme modu kapalı **ve** açık raporlanır. Mod C varsayılanı gecikme için düşünme kapalıdır.
- **İnce ayar riski (Faz 5):** model çok kipli sarmalayıcıyla geliyor (`Gemma4ForConditionalGeneration`) ve PLE tabloları var; LoRA yalnızca metin çözücüsüne uygulanacak. Araç desteği (ör. Unsloth sürümleri) mevcut görünüyor ama **Faz 0'da küçük bir deneme ince ayarıyla doğrulanmalı**.
- **Mod C riski:** 8B toplam ağırlık tarayıcıda indirme boyutunu belirler; `q4f16` dosya boyutları Faz 0'da ölçülmeli. Çok büyükse Mod C "isteğe bağlı indirme" olarak sunulur.
- Diğer 3–4B modeller **desteklenir ama hedeflenmez**: kart ve sözdizimi Gemma 4'e göre ayarlanır, diğerleri regresyon göstergesidir.

### 9.1 Kullanım modları

| Mod | Model | Mekanizma |
|---|---|---|
| **A — Sıfır ince ayar** | Gemma 4 E4B (referans); diğer 3–4B+ modeller | `card.md` + birkaç örnek + GBNF kısıtlı üretim + hata döngüsü |
| **B — İnce ayarlı** | Tez-Coder = Gemma 4 E4B ince ayarı (açık ağırlık, Apache-2.0) | LoRA/tam ince ayar; kart gerekmez |
| **C — Tamamen tarayıcıda** | Tez-Coder, `q4f16` ONNX | WebGPU (Transformers.js) + GBNF; sunucu yok, ağ yok |

Mod C, "tamamen tarayıcıda geliştirme" hedefinin uç noktasıdır: editör, derleyici, doğrulayıcı, runtime **ve model** kullanıcının cihazında.

### 9.2 Eğitim seti üretim hattı

Derleyici bir **mükemmel doğrulayıcı** olduğundan, veriyi büyük ölçüde sentetik ve otomatik doğrulanmış üretebiliriz:

1. **Tohum görevler:** Açık lisanslı problem setleri (MBPP, HumanEval tarzı) + TEA UI görevleri; doğal dil açıklaması + testler.
2. **Üretim:** Büyük bir model, `card.md` ile her görev için N aday Tez programı üretir.
3. **Reddetmeli örnekleme:** Yalnızca `tez check` (tür + doğrulama) ve testleri geçenler tutulur.
4. **Kanonikleştirme:** Tüm örnekler `tez fmt`'den geçer → veri setinde tek bir stil.
5. **Düzeltme izleri:** Başarısız adaylar atılmaz; `(hatalı kod, JSON hata) → düzeltilmiş kod` çiftleri olarak saklanır. Model yalnızca yazmayı değil, **döngünün kendisini** öğrenir.
6. **Sözleşme zenginleştirme:** Her fonksiyona `ensures` eklenmiş varyantlar; model sözleşme yazmayı alışkanlık edinir.
7. **Mutasyon verisi:** Doğru programlara kontrollü hatalar enjekte edilir (sınır, sıfıra bölme, eksik kol) → hata/düzeltme çiftleri.
8. **Ayrık test seti:** TezBench görevleri eğitimde asla kullanılmaz (kirlenme kontrolü).

### 9.3 TezBench

- 100+ görev: saf fonksiyonlar, veri dönüşümleri, TEA uygulamaları; her biri testli ve sözleşmeli.
- Karşılaştırma: aynı görevlerin TypeScript sürümleri, aynı modellerle.
- Model sınıfları: **Gemma 4 E4B (ana hedef)**, bir ikincil küçük model (regresyon göstergesi), bir öncü model.

| Metrik | Tanım | Hedef (Faz 0'da kalibre) |
|---|---|---|
| Sözdizimi geçerliliği | İlk denemede parse olan çıktı | GBNF ile %100; GBNF'siz, Mod A 3–4B ≥ %90 |
| pass@1 (Mod A, 3–4B) | İlk denemede tüm kontrolleri geçen | TypeScript ile eşit veya üstü |
| pass@1 (Mod B, 3–4B) | İnce ayarlı | Mod A'nın belirgin üstü |
| Döngü yakınsaması | ≤ 3 turda geçen | ≥ %90 |
| Token oranı | Tez / TS token sayısı | ≤ 0,8 (Gemma 4 tokenizer'ında; ikincillerde raporlanır) |

### 9.4 Tokenizer kümesi

**Birincil:** Gemma 4 (`google/gemma-4-E4B`, `tokenizer.json`, 262K sözlük). Tasarım kararları buna göre verilir; tek-token kuralı ve token oranı hedefi burada **bağlayıcıdır**.

**İkincil:** iki farklı aileden birer tokenizer (seçim Faz 0'da). Yalnızca raporlanır; Tez'in tek bir tokenizer'a aşırı uyum sağlamadığını göstermek içindir. İkincilde belirgin kötüleşme varsa karar gözden geçirilir ama otomatik reddedilmez.

## 10. Yapılandırılmış Hata Şeması

```json
{
  "version": 1,
  "status": "error",
  "diagnostics": [{
    "code": "E_REFINEMENT",
    "phase": "verify",
    "message": "Bölen sıfır olabilir",
    "span": { "start": { "line": 7, "col": 14 }, "end": { "line": 7, "col": 19 } },
    "expected": "len(xs) != 0",
    "counterexample": { "xs": "[]" },
    "fixes": [{
      "title": "Ön koşul ekle",
      "edits": [{ "span": { "start": { "line": 6, "col": 33 }, "end": { "line": 6, "col": 33 } },
                  "replace": "\n  requires len(xs) > 0" }]
    }]
  }]
}
```

- Bir düzenleme `span` aralığını `replace` metniyle değiştirir; boş aralık = ekleme.
- Kararlı kodlar: `E_PARSE_*`, `E_TYPE_*`, `E_MATCH_*`, `E_REFINEMENT`, `E_UNPROVEN`, `E_TERMINATION`, `E_RESOURCE`.
- Hata kurtarmalı parser → tek derlemede tüm hatalar.
- Mesajlar kısa ve kalıplı — küçük modelin bağlamına az token ekler; eğitim setinde aynı kalıplar kullanılır.

## 11. Konumlanma

| Proje | İlişki | Tez'in farkı |
|---|---|---|
| **AILang (Sunholo)** | AI tarafından yazılan kod için saf fonksiyonel dil; açık etkiler | Tez: SMT ile kanıtlanmış hata sınıfları, 3–4B hedefi + açık eğitim seti/model, WasmGC + tamamen tarayıcıda geliştirme |
| **MoonBit** | AI-dostu, WASM öncelikli | Tez: biçimsel doğrulama, küçük model odaklı minimal yüzey |
| **Elm** | Saf, TEA, çalışma anı hatası yok | Tez: refinement/sözleşme, LLM döngüsü, WASM |
| **Dafny / F\*** | SMT tabanlı doğrulama | Tez: web uygulaması runtime'ı, LLM-öncelikli sözdizimi, tarayıcıda çalışma |
| **v0 / Bolt / Lovable** | Prompt → web uygulaması ürünleri | Tez: garanti eden derleyici; tamamen yerel çalışabilen açık yığın |

**Tek cümle:** *Tez, küçük bir modelin bile tarayıcıda yazıp anında çalıştırabildiği ve derlendiyse belirli hataları içeremeyeceği kanıtlanmış dildir.*

## 12. Yol Haritası

| Faz | Süre | Çıktı | Çıkış kriteri |
|---|---|---|---|
| **0. Dil tasarımı + ölçüm** | 2 hafta | `grammar.ebnf/.gbnf`, `card.md`, 30 örnek program, tokenizer ölçüm aracı, TezBench iskeleti (20 görev) | Mod A, bir 3–4B modelde ≥ %80 sözdizimi geçerliliği; token oranı ≤ 0,9 |
| **1. Çekirdek** | 3 hafta | lexer, parser, ad çözümleme, tür denetimi, kapsamlılık, `tez_interp`, JSON hatalar, `tez fmt`, CLI | §5 Faz-1 sınıfları testlerle kanıtlı |
| **2. WasmGC + tarayıcı** | 3 hafta | `tez_wasm`, `tez_web`, Studio (Monaco), TEA runtime, Worker, fuel, hot-swap | Sayaç örneği tarayıcıda canlı; §7 bütçeleri (L1 hariç) tutuyor |
| **3. LLM döngüsü + veri hattı v1** | 2 hafta | Studio'da LLM entegrasyonu, otomatik düzeltme, §9.2 hattının ilk sürümü | **İlk uçtan uca demo**; TezBench ilk ölçüm |
| **4. Doğrulama** | 4 hafta | refinement, VC üretimi, `tez_solve` (L1), Z3 (L2), karşı örnekler, `decreases`, VC önbelleği | §5'teki tüm sınıflar aktif; L1 < 1 ms/VC |
| **5. Tez-Coder v0** | 3 hafta | Eğitim seti v1 (doğrulanmış + düzeltme izleri), 3–4B ince ayar, açık ağırlık yayını | Mod B pass@1, Mod A'nın belirgin üstü |
| **6. Tarayıcı içi model** | 2 hafta | Mod C: WebGPU + GBNF Studio'da | İnternetsiz uçtan uca demo |
| **7. Etkiler** | sonra | `Cmd`/`Sub`, HTTP, depolama | — |

## 13. Açık Sorular

1. ~~Ad kontrolü~~ **Karar: marka Tez, paket/depo `tez-lang`** (yayında, 2026-10-14). Açık: domain.
2. ~~Referans model~~ **Karar: Gemma 4 E4B** (§9.0). Açık: iki ikincil tokenizer ailesi, Mod C indirme boyutu, LoRA denemesi.
3. **Veri üretimi için büyük model:** §9.2 adım 2'de hangi model/API kullanılacak? (Çıktı lisans koşulları kontrol edilmeli.)
4. **Studio dağıtımı:** Statik barındırma (COOP/COEP başlıkları destekli) — hangi platform?
