# Tez — Geliştirme Notları

> Güncelleme: 2026-10-15 (inceleme + ölçüm turu; Faz 8/9 altyapı eklendi, bkz. §7) · Durum: tüm Rust/Node testleri, gramer kontrolü ve TezBench referansları yeşil
> Kaynaklar: `docs/ROADMAP.md`, `spec/`, `bench/results/`. Bu belge analiz ve öneri özetidir; iş durumunun kaynağı `ROADMAP.md` kalır.
> Kural: buradaki her sayı bir komutla yeniden üretilebilir olmalı (komutlar her başlıkta).

## 1. Proje durumu özeti

- 10 crate'lik Rust derleyici hattı: `tez_syntax` → `tez_types` → (`tez_verify` | `tez_interp` | `tez_wasm`), destek: `tez_diag` (61 kararlı hata kodu), `tez_fmt`, `tez_driver`, `tez_web`, `tez_cli` (paket `tez-lang`).
- Faz 1, 2 ve 4 ✅; Faz 0/3/5 araçları hazır ve ilk model ölçümleri yapıldı; Faz 6 kodu yazıldı ama uçtan uca denenmedi; Faz 7'den yalnız `Sub.every` (zaman) var.
- Doğrulama tarayıcıda da çalışıyor (Z3 WebAssembly + replay arka ucu). Kanıt önbelleği, kuyruk çağrıları, olay günlüğüyle hot-swap, HTML sandbox mevcut.

## 2. İnceleme bulguları ve düzeltmeler (2026-10-15)

Projenin 1 numaralı ilkesi ("kanıtlanamayan reddedilir") iki yerde **ihlal ediliyordu**; ikisi de düzeltildi ve regresyon testleri var.

| Bulgu | Önem | Düzeltme |
|---|---|---|
| `Str.upper`/`lower` sözleşmesi `result.len() == self.len()` diyordu; `"ß".upper()` = `"SS"`. Doğrulanmış bir program sıfıra bölmeyle çöküyordu | kritik | Sözleşme kaldırıldı |
| `Float` taşması ∞/NaN üretiyordu; doğrulayıcı tam sıralı float varsaydığı için doğrulanmış `assert x == x` çalışırken başarısız oluyordu | kritik | Taşma her yerde (yorumlayıcı, WASM, runtime) `E_RESOURCE`; sonlu olmayan float literali reddedilir; NaN/∞ hiç oluşmaz |
| Bir Tez programı `Html.node("script")`, `on*` öznitelikleri ve `javascript:` bağlantılarıyla sayfada JS çalıştırabiliyordu (Studio'da API anahtarı aynı sayfada) | kritik (güvenlik) | `tez-app.mjs` sandbox: izinli etiketler, `on*`/`srcdoc` düşer, URL'ler yalnız http(s)/mailto/göreli |
| Güvenilen `extern` sözleşmeleri zor girdilerle test edilmiyordu | orta | Her sözleşme Unicode/ZWJ/büyük sayılarla hem yorumlayıcıda hem WASM runtime'da test ediliyor; **bu test bir hata daha buldu**: `List.range` çok büyük sınırlarda kısa liste için bile `E_RESOURCE` veriyordu (düzeltildi) |
| `b.f(0)` (fonksiyon tipli alan) yanıltıcı hata veriyordu | düşük | Mesaj ve otomatik düzeltme `(b.f)(..)` öneriyor |
| Kanıt önbelleğine yazabilen biri sorguyu "kanıtlanmış" gösterebilir | not | Güven modeli `language.md`'de belgelendi; `TEZ_NO_CACHE` |

Sağlam bulunanlar: ~45 karşı-örnek programda (indeks, bölme, `decreases` hileleri, fonksiyon değeri/lambda üzerinden özyineleme, kaçan lambdalar, sabitler) doğrulayıcı doğru reddetti; yorumlayıcı ile WASM 17 uç durumda birebir aynı.

## 3. Ölçümler

### 3.1 Token ekonomisi — `cd bench/tokens && node measure.mjs`

| Metrik | Değer |
|---|---|
| Tez/TS oranı (28 eşli program) | **0,88** |
| Kanıt satırları hariç | 0,84 |
| Tek token anahtar kelime/operatör | 36/38 (`ensures`, `decreases` satır başında 2 token) |
| Dil kartı | 1.123 / 1.200 token |

Not: bu belgenin önceki sürümündeki 0,99 oranı ve 930 token kart güncel değil. Oran ilk 14 programda 0,82 idi; örnek arttıkça 0,88'e yükseldi. Kazanç ADT/kayıt ağırlıklı kodda, kayıp kısa ve sözleşmeli fonksiyonlarda (ör. `17-gcd` 1,43).

### 3.2 TezBench — `node bench/tezbench/generate.mjs …` + `node bench/tezbench/run.mjs --candidates DIR`

20 görev, Gemma 4 E4B (LM Studio), "geçer" = doğrulanır + bütün vakaları geçer.

| Koşu | Örnekleme | sözdizimi geçerli | geçer |
|---|---|---|---|
| Tez, kart yok | greedy | %0 | %0 |
| Tez, kart (ilk sürüm) | greedy, 1 örnek | %75 | %30 |
| Tez, kart v2 | greedy, 1 örnek | %85 | %65 |
| Tez, güncel kart | **T=0,7, 3 örnek** | %75 / %80 / %80 | **%50 / %50 / %50** |
| **mlx-serve, `mlx-community/gemma-4-e4b-it-4bit`** — kart A (çok dosya satırından önce, 1.148 token) | **T=0,7, 5 örnek** | %78 | **%52** (50–55) |
| aynı model, kart B (+ çok dosya satırı) | T=0,7, 5 örnek | %82 | **%53** (50–55) |
| aynı model, **güncel kart** (+ `Sub.clock/online/visible`, 1.198 token) | T=0,7, 5 örnek | %75 | **%46** (45–50) |
| aynı model, güncel kart − `Sub.clock/online/visible` cümlesi | T=0,7, 5 örnek | %72 | **%49** (45–55) |
| Tez, kart + 3 tur derleyici geri bildirimi | greedy | %90 | %55 |
| Phi-4-mini-reasoning, kart | greedy | %75 | %5 |
| Gemma 4 E4B + küçük LoRA, kart yok | greedy | %80 | **%70** |
| aynı, GBNF kısıtlı üretim | greedy | %95 | %70 |
| TypeScript (aynı görevler) | greedy | — | %100 |

**Kart değişikliğinin maliyeti (2026-10-22, mlx-serve 4-bit Gemma 4 E4B, 20 görev × 5 örnek, T=0,7):** eski kartlar %52–53, güncel kart %46. LM Studio'daki eski %50/%50/%50 ile tutarlı (aynı model, farklı sunucu/kuantizasyon). Düşüş birkaç göreve yoğunlaşıyor: `18-gcd` 4/5 → 0/5 (model `decreases b` satırını unutuyor), `04-count-positive` 4/5 → 1/5, `08-fibonacci` 5/5 → 3/5; hiçbir kartla geçmeyen 8 görev değişmedi. `Sub` cümlesi geri alınınca %49'a çıkıyor, yani etkinin bir kısmı cümleden, bir kısmı kısaltılan `use` satırından ya da gürültüden (örnek kümesi sapması ≈ ±2–4 puan); kesin ayrıştırma bu örnek sayısıyla mümkün değil. **Karar:** canlı siteler birinci hedef olduğu için `Sub.clock/online/visible` kartta kalıyor; maliyet saf fonksiyon görevlerinde ≈ 6 puan. Düzeltme adayı: web/uygulama istemlerine kart eki, saf görevlere çekirdek kart (kart ≤ 1.200 kuralı nedeniyle çekirdek küçülmeden eklenemez) — ayrı karar. Sonuçlar: `bench/results/tezbench-mlx-card{A,B,C,D}-s{1..5}-2026-10-22.json`.

**Ölçüm dersi:** tek greedy örnekle 20 görev ±%20 oynuyor. Kart v2'nin %65'i iyimser bir tek örnekti; 3 örnekli ölçüm %48–50. Bundan sonra karşılaştırmalar **en az 3 örnek, T=0,7** ile yapılmalı ve aynı adaylar eski/yeni derleyiciyle yeniden yargılanmalı (`TEZ_BIN=…`).

### 3.3 Hız — `tez build` + `node runtime/run.mjs`

`fib(35)`: derlenmiş Tez ≈ 0,09 s, düz JS 0,05 s (~2×). Kutusuz fast-path ertelendi.

## 4. Öneriler: ne yapıldı, ne reddedildi

Değerlendirme yöntemi: kayıtlı tüm model çıktılarının (~200 aday) **ilk hata kodu** dağılımı.

### Benimsendi (2026-10-15)

1. **`decreases` çıkarımı** (önceki B.4). Kendi kendini çağıran fonksiyonlarda ölçü çıkarılır: `[h, ..t]` kuyruğu → `p.len()`; toplam türün alt değeri → `p`; tuple eşleşmesindeki bileşenler; literalle geri sayım `f(n - k)` → `n`. Ölçüyü yine doğrulayıcı kanıtlar (sağlam); kanıtlanamazsa hata "`decreases` yazılmadı, çıkarılan ölçü…" der. Karşılıklı özyineleme ve yukarı sayma hâlâ `decreases` ister.
   - Kartta **anlatılmıyor**: "çıkarılır" cümlesi eklenince model Int özyinelemesinde de `decreases`'i atladı ve sonuç düştü. Çıkarım sessiz bir emniyet ağı olarak kalıyor.
   - Etki (aynı 60 aday, eski → yeni derleyici): %48,3 → %50,0. Küçük, çünkü bu kartla model çoğu zaman metot (`map/filter/sum`) yazıyor; kalan `decreases` hataları hatalı desenlerde (`[h, t]` = iki elemanlı liste).
2. **`fn f() -> T { … }` gövdesi** kabul edilir ve `tez fmt` bunu `= { … }` yapar (tek kanonik biçim korunur). `return x;` için ayrı açıklayıcı hata ve otomatik düzeltme var. Kartsız çıktıların ana ayrıştırma hatası buydu; ama kartsız çıktılar `return`, `;`, `snake_case` da içerdiği için tek başına geçme oranını değiştirmedi.

### Ölçüldü, reddedildi ya da ertelendi

- **"GBNF kısıtlı üretim tek başına en büyük getiri"**: ölçüldü. Sözdizimi %80 → %95, geçme %70 → %70. Darboğaz anlam (tür/sözleşme), sözdizimi değil.
- **Match kollarında `,`**: verideki sıklık ~1/60 → şimdilik hayır (hata + otomatik düzeltme zaten var).
- **Dizgide `+`**: güncel ölçümde 2/60 (`E_TYPE_BAD_OPERAND`). İzlemede; lambdalarda tür belirsizliği riski yüzünden henüz değil.
- **Interpolasyon, `?`, `is`**: veride sinyal yok; ölçmeden ekleme.
- **`extern`'i kaldırma**: prelude kullanıyor; kalır.
- **Kutusuz fast-path**: ölçüm darboğaz göstermiyor.

### Yeni adaylar (güncel 60 adaydaki hatalar)

| Hata | Adet | Olası çözüm |
|---|---|---|
| `clamp(x, 0, 100)` (metodu fonksiyon gibi çağırma) | 3 | `E_NAME_UNKNOWN` yerine "`x.clamp(0, 100)` mı?" düzeltmesi (ucuz, dil değişmez) |
| `'a'` tek tırnaklı karakter | 3 | Hata mesajı + otomatik `"a"` düzeltmesi |
| `s[0]` dizgide indeks | 2 | Mesajda `s.slice(i, i + 1)` önerisi |
| `[h, t]` ile cons deseni sanma | 2 | Uyarı yok; derleyici mesajında "`[h, ..t]` mi?" ipucu |
| `\|x\| …` ve `_ > 0` lambda biçimleri | 2 | Önce ölç; `x => …` tek yol kalmalı |

Hepsi **dili değiştirmeyen** tanı/düzeltme iyileştirmeleri; tamir döngüsünün (+%25) etkisini artırır.

## 5. Sıradaki somut adımlar

1. Yukarıdaki tanı/düzeltme adaylarını ekle, aynı 60 adayı ve tamir döngüsünü yeniden ölç.
2. TezBench'in yanında **ayrı, dışlanmış bir görev kümesi** (kart tasarımı TezBench hatalarına bakılarak değiştirildiği için TezBench artık hafif sızıntılı).
3. 7–8B sütunu: Granite 4.1 8B (kullanıcı indiriyor) — 3 örnekli protokolle.
4. LoRA'yı genişlet (daha çok örneklenmiş program + kartlı istemlerle) ve kartlı kipte düşüşü gider.

## 6. Ortam notu

`make check` wasm32 hedefinde `E0463: can't find crate for std/core` ile düşerse: Homebrew sade `rust` formülü rustup shim'lerini gölgeliyordur. Düzeltme: `brew unlink rust && brew link --force rustup` (geri alma: `brew link rust`). `rust-toolchain.toml` (1.99.0 + wasm32) doğru.

## 7. Altyapı turu (2026-10-15): birinci hedef "tarayıcıda canlı siteler"

Yapılanlar (ayrıntı ve durum: `docs/ROADMAP.md` Faz 8–9):
- **Çok dosyalı programlar** (`use "x.tez"`), CLI + `tez_web` proje API'si + Studio dosya sekmeleri (proje tarayıcıda saklanır).
- **npm paketi `tez-lang`** (`npm/tez-lang`): derleyici WebAssembly olarak, `tez check|run|test|build|site|fmt`; kanıt için sistem `z3` ya da `z3-solver` WASM. **Hazır ama yayımlanmadı**; `release.yml` etiketle TASLAK release üretir (5 platform).
- **Siteyi dışa aktarma**: Studio *Publish* ve `tez site` → tek dosya (wasm + runtime + host gömülü, sıkı CSP).
- **Tetikleyiciler**: `Sub.every/clock/online/visible` (cron benzeri zamanlama saf kod). **Kalıcılık**: `save`/`restore`. **Ağ**: `fetches` → `Fetch.get`, yalnız izinli kökenler.
- Studio model döngüsü kanıt hatalarını da modele geri verir.

Dikkat: dil kartı tam **1.200/1.200 token** (yeni `Sub` cümlesi 1.148 → 1.200); `Fetch`/`save`/`restore` kartta yok (gelişmiş özellik; `language.md`'de). Kart değiştiği için TezBench karşılaştırmaları (§3.2) yeniden ölçülmeli; şu anki sayılar eski kartla.

Açık: gerçek WebGPU model denemesi, `tez_web` ile `Fetch` için POST/başlık, yayın/barındırma bağlantısı, VS Code eklentisi/LSP, `tez check --watch`/`tez new`.


### 7.1 Review düzeltmeleri (2026-10-16)
- **Site CSP**: `img-src`/`media-src` artık yalnızca `data:` + sayfanın kökeni + `--allow` kökenleri (önceden her `https:` açıktı → izin listesi `<img>` ile aşılabiliyordu). Başlık/CSS `</script>`/`</style>` ile elemandan çıkamaz; `allow` yalnız http(s), wasm yalnız base64.
- **Fetch**: gövde akışla okunur, 1 MB (byte) aşılınca okuma kesilir; `stop()` uçuştaki istekleri iptal eder.
- **Hot-swap logu**: dolunca program `save`/`restore` tanımlıysa o anki durum checkpoint olur (log sıfırlanır); tanımlı değilse kaydedilemeyen olaylar sayılır ve swap bunları `dropped` olarak bildirir (önceden sessizce farklı duruma replay ediyordu). Geri yüklenmiş (restore) bir oturumda swap artık `init()` yerine o durumdan başlar; yeni sürüm okuyamazsa `restarted: true`.
- **Dosya başına görünürlük** (`E_USE_MISSING`): bir dosya yalnızca kendi adlarını ve doğrudan `use` ettiği dosyalarınkini görür (geçişli değil). Tip denetiminden sonra ayrı bir geçiş (`tez_driver/src/visibility.rs`), yalnızca reddeder; fix `use` satırını ekler, döngü yaratacaksa taşımayı önerir. Kart: 1.198/1.200.


## 8. CLI turu ve gerçek model (2026-10-22)
- **Dokümanlar İngilizce**: sitede görünen `docs/ARCHITECTURE.md` ve `docs/ROADMAP.md` İngilizceye çevrildi (site artık tamamen İngilizce; `DEVELOPMENT.md` sitede yok, Türkçe kaldı). Açılış sayfası güncellendi (kart 1.198 token, 64 hata kodu, ~300 KB derleyici, "Sites that run themselves" bölümü), README ve GitHub deposu açıklaması/konuları güncellendi.
- **CLI** (yerel ikili ve npm sürümü aynı davranış, ikisi de testli): `--help` çıkış kodu 0, komutsuz `tez` 2; bilinmeyen bayrak reddedilir; `--watch`; `tez new <dir> [--app]` (`spec/templates/`); `tez fmt <dir>`. VS Code/LSP ertelendi.
- **İnce ayar veri seti v2** (`data/v2/`, ayrıntı `data/v2/README.md`): 8.322 örnek (8.014 eğitim / 308 doğrulama): 3.166 saf fonksiyon (78 aile + 683 çok fonksiyonlu program; JSON, tarih/saat dahil), 1.603 web uygulaması (49 aile: zamanlayıcı, saat, fetch, JSON akışı, POST, save/restore), 3.431 onarım (24 bozma türü, derleyicinin gerçek tanıları; 571'i kanıt hatası), 122 hata kodu açıklaması. Her program derleyicide kanıtlandı ve testleri çalıştı; bağımsız denetim (`audit.mjs`) tümünü yeniden kanıtlıyor/reddediyor, TezBench ve web bench sızıntısı yok. İnce ayarı kullanıcı yapacak (Unsloth, 3B); tarif README'de. Değerlendirme: TezBench `tez-nocard` + `web.mjs --example none`.
- **Web benchmark** (`bench/tezbench/web.mjs`, 8 tutulan sayfa görevi): ayarsız Gemma 4 E4B (4-bit) örneksiz %0, tam örnek uygulama istemde %19 (2 örnek × 2 tur); model JSX benzeri söz dizimi uyduruyor. Studio artık web isteklerinde `15-counter-app` örneğini istemin sonuna ekliyor (`wantsApp`).
- **Bağımsız site denemesi ve eksikler** (`examples/hn-live`): Hacker News ilk 5 haberini her 5 dakikada bir (GitHub Actions cron'u haberleri `stories.tez`'e yazıp Tez ile kanıtlayıp derler; sayfa açıkken de `Sub.clock` ile tarayıcıda yenilenir) gösteren site; tarayıcıda doğrulandı (08:25 işaretinde `round=1` istekleri gitti). Eksikler ROADMAP Faz 10'da: JSON ayrıştırıcı (`Json`, prelude'da Tez ile yazıldı ve kanıtlandı), `Fetch.json/post/postJson/send`, `Str.toFloat`, `Char.fromCode`, `Str.replace/indexOf`, `Time.*`, `Html.link/list/ordered`, prelude türlerinde metot çağrısı, `tez site --css/--title`. Kart 1.199/1.200: yeni modüller `spec/language.md`'de belgeli (test zorluyor), ince ayar veri setinde öğretiliyor. **Not:** `tez-lang` 0.1.0 bu özellikleri içermiyor; yayın için 0.2.0'a geçildi.
- **Gerçek model**: mlx-serve (`mlx-community/gemma-4-e4b-it-4bit`, OpenAI uyumlu, `:11234`). Uygulamanın sunucusunda başka bir model yüklü olduğu için GPU belleği dalgalanıyordu (istekler `prefill refused` ile reddedildi → yarım kalan koşular 0 görünür). Çözüm: ikinci bir örnek `mlx-serve --serve --port 11235 --skip-mem-preflight --no-vision --ctx-size 4096 --kv-quant 8 --prefill-chunk 256 --os-reserve-gib 1`; `bench/tezbench/generate.mjs` artık geçici hatada yeniden dener ve `--card DOSYA` ile kartın başka bir sürümünü ölçer.
- **0.2.1 (inceleme düzeltmeleri)**: `Json.toText` artık U+0000–U+001F arasındaki her kontrol karakterini kaçışlıyor (önceden geçersiz JSON üretebiliyordu); eşi olmayan surrogate yarımları U+FFFD oluyor, araya metin giren yarımlar birleştirilmiyor; JSON okuyucunun `JsonTok`/`JsonFrame` tipleri kaldırıldı (tuple), böylece `TkStr`, `FrArr` gibi adlar programlara açık. Testler: `spec/examples/57-json.tez` ve `prelude_types_leave_names_free_for_programs`. Açık kalan: okuyucu fazla hoşgörülü (`[1 2]`, `[1,]`, `01`), `1e400` reddediliyor, istek anahtarı başlıkları içermiyor.
