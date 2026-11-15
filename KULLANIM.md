# EnesLauncher — düzen ve bakım

## Klasör düzeni

```
EnesLauncher\               ← kaynak kod (git deposu)
├─ apps\desktop\            React + Vite arayüzü, node_modules
├─ crates\                  14 Rust crate
├─ target\                  derleme cache'i (git'e girmez, silinebilir)
├─ .cargo\config.toml       (opsiyonel) cache'i başka diske taşımak için
├─ build.ps1                tek komutla derle → sabit exe
└─ temizle.ps1              cache bakımı (exe'ye dokunmaz)

..\EnesLauncher-App\        ← SABİT ÇIKTI, hep burada kalır
└─ EnesLauncher.exe
```

Sabit exe proje klasörünün **bir üstünde** durur, `target\` içinde değil.
Yani `cargo clean` çalıştırsan da `EnesLauncher-App\EnesLauncher.exe` yerinde kalır.

Cache'i proje dışında tutmak istersen:

```powershell
Copy-Item .cargo\config.toml.example .cargo\config.toml
# içindeki target-dir satırını aç ve kendi yolunu yaz
```

## Günlük kullanım

| Ne yapmak istiyorsun | Komut |
|---|---|
| Kodu değiştirdim, exe'yi güncelle | `.\build.ps1` |
| Derle + hemen aç | `.\build.ps1 -Run` |
| Hızlı derle (debug, yavaş çalışır) | `.\build.ps1 -Dev` |
| Sadece Rust (arayüz değişmedi) | `.\build.ps1 -SkipFrontend` |
| Sadece arayüz geliştir | `npm run tauri dev` (apps\desktop içinde) |
| Disk şişti | `.\temizle.ps1` |
| Cache tamamen bozuldu | `.\temizle.ps1 -Hepsi` |

`build.ps1` her çalıştığında masaüstündeki `EnesLauncher.lnk` kısayolunu da
sabit exe'ye yönlendirir. Kısayolun bozulması imkansız hale gelir.

## Boyutlar

| | Önce | Sonra |
|---|---|---|
| `target/` (debug) | 14,66 GB | ~1,6 GB |
| `target/` (release) | — | ~1,8 GB |
| Kaynak klasörü (cache hariç) | — | ~0,15 GB |
| Sabit exe | 26 MB (debug) | ~10 MB (release) |

## Derleme ayarları (`Cargo.toml`)

```toml
[profile.dev]
debug = "line-tables-only"   # tam sembol yerine satır bilgisi

[profile.dev.package."*"]
debug = false                # bağımlılıklar sembolsüz → rlib'ler %60-70 küçük
opt-level = 1

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
```

Eskiden hiç `[profile]` yoktu; dev profili tam debug sembolü üretiyordu
(`libwindows.rlib` tek başına 117 MB, `enes_launcher_app.pdb` 175 MB).
Cargo eski artefaktları silmediği için 2 günde 14,66 GB birikmişti.

## Loglar

Tek dosya: `%LOCALAPPDATA%\eneslauncher\EnesLauncher\data\logs\launcher.log`

- 512 KB'a ulaşınca sıfırdan başlar (asla şişmez)
- 7 günden eski loglar açılışta silinir
- Sadece panik ve kritik hata yazar
- Eski `launcher_panic / launcher_boot / launcher_setup / launcher_tauri_error`
  dosyaları kaldırıldı

`data\instances` ve `data\shared` **oyun verisidir** (1,9 GB), log değil.
Yer açmak için launcher arayüzünden kullanılmayan profilleri sil.
