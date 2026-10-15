# Tez — Geliştirme Notları

> Güncelleme: 2026-10-15 (inceleme + ölçüm turu) · Durum: tüm Rust/Node testleri, gramer kontrolü ve TezBench referansları yeşil
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
| Tez, kart + 3 tur derleyici geri bildirimi | greedy | %90 | %55 |
| Phi-4-mini-reasoning, kart | greedy | %75 | %5 |
| Gemma 4 E4B + küçük LoRA, kart yok | greedy | %80 | **%70** |
| aynı, GBNF kısıtlı üretim | greedy | %95 | %70 |
| TypeScript (aynı görevler) | greedy | — | %100 |

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
