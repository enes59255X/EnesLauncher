use launcher_core::LauncherResult;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrashSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashDiagnosis {
    pub title: String,
    pub severity: CrashSeverity,
    pub faulty_mod: Option<String>,
    pub cause: String,
    pub evidence: String,
    pub recommendation: String,
    pub confidence_score: f32, // 0.0 - 1.0
}

pub struct CrashAnalyzer;

impl CrashAnalyzer {
    /// Crash raporu veya log içeriğini kanıta dayalı olarak derinlemesine analiz eder
    pub fn analyze_log(log_text: &str) -> LauncherResult<CrashDiagnosis> {
        // 1. Senaryo: Bellek Yetersizliği (OutOfMemoryError)
        if log_text.contains("java.lang.OutOfMemoryError")
            || log_text.contains("Java heap space")
            || log_text.contains("Metaspace")
        {
            let evidence = log_text
                .lines()
                .find(|l| l.contains("OutOfMemoryError") || l.contains("Java heap space") || l.contains("Metaspace"))
                .unwrap_or("java.lang.OutOfMemoryError: Java heap space")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Yetersiz RAM / Bellek Aşımı (Heap Space Doldu)".to_string(),
                severity: CrashSeverity::Critical,
                faulty_mod: None,
                cause: "Minecraft ve kurulu modlar için ayrılan JVM bellek sınırı (RAM) tükendi.".to_string(),
                evidence,
                recommendation: "Profil ayarlarından ayrılan belleği artırın. Modlu paketler için en az 6144 MB (6 GB) veya 8192 MB (8 GB) RAM tahsis edin.".to_string(),
                confidence_score: 0.99,
            });
        }

        // 2. Senaryo: Ekran Kartı / OpenGL / GLFW Hatası
        if log_text.contains("GLFW error 65542")
            || log_text.contains("Pixel format not accelerated")
            || log_text.contains("WGL: The driver does not appear to support OpenGL")
            || log_text.contains("org.lwjgl.glfw.GLFWErrorCallback")
        {
            let evidence = log_text
                .lines()
                .find(|l| l.contains("GLFW error") || l.contains("Pixel format") || l.contains("OpenGL"))
                .unwrap_or("GLFW error 65542: WGL: The driver does not appear to support OpenGL")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Grafik Sürücüsü / OpenGL Hatası (GLFW 65542)".to_string(),
                severity: CrashSeverity::Critical,
                faulty_mod: None,
                cause: "Ekran kartı sürücünüz Minecraft'ın gerektirdiği OpenGL donanım hızlandırmasını sağlayamıyor veya sürücü güncel değil.".to_string(),
                evidence,
                recommendation: "Ekran kartı (NVIDIA / AMD / Intel) sürücülerinizi resmi üretici yazılımından güncelleyin. Dizüstü bilgisayar kullanıyorsanız oyunun harici yüksek performanslı GPU ile çalıştığından emin olun.".to_string(),
                confidence_score: 0.98,
            });
        }

        // 3. Senaryo: Java Modül Çakışması (ResolutionException - Modules export package to module...)
        if log_text.contains("java.lang.module.ResolutionException")
            || log_text.contains("export package") && log_text.contains("to module")
        {
            let evidence = log_text
                .lines()
                .find(|l| l.contains("ResolutionException") || l.contains("export package"))
                .unwrap_or("java.lang.module.ResolutionException: Module collision")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Java Platform Modül Sistemi (JPMS) Çakışması".to_string(),
                severity: CrashSeverity::Critical,
                faulty_mod: None,
                cause: "İki farklı JAR dosyası veya kütüphane aynı Java paketini (package) ihraç etmeye çalıştı. Genellikle vanilla JAR'ın ignore listesine girmemesinden kaynaklanır.".to_string(),
                evidence,
                recommendation: "EnesLauncher'ın en son derlemesinde bu sorun otomatik olarak çözüldü. Oyunu doğrudan başlatmayı tekrar deneyin.".to_string(),
                confidence_score: 0.97,
            });
        }

        // 4. Senaryo: Java Sürüm Uyuşmazlığı (UnsupportedClassVersionError)
        if log_text.contains("UnsupportedClassVersionError")
            || log_text.contains("has been compiled by a more recent version of the Java Runtime")
        {
            let evidence = log_text
                .lines()
                .find(|l| l.contains("UnsupportedClassVersionError"))
                .unwrap_or("java.lang.UnsupportedClassVersionError")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Uyumsuz Java Sürümü (JDK Hatası)".to_string(),
                severity: CrashSeverity::Critical,
                faulty_mod: None,
                cause: "Kurulu modlardan veya oyun çekirdeğinden biri, bilgisayarınızdaki Java sürümünden daha yeni bir Java gerektiriyor.".to_string(),
                evidence,
                recommendation: "Minecraft 1.20+ sürümleri için sisteminizde en az Java 17 veya Java 21 kurulu olmalıdır. Ayarlar kısmından Java yolunuzu güncelleyin.".to_string(),
                confidence_score: 0.98,
            });
        }

        // 5. Senaryo: Mixin Kod Çakışması (MixinApplyError)
        if log_text.contains("MixinApplyError")
            || log_text.contains("org.spongepowered.asm.mixin.transformer.throwables")
            || log_text.contains("Critical injection failure")
        {
            let re_mod = Regex::new(r"Mixin \[(?P<mod>[^:]+)\.mixins\.json").unwrap();
            let mut faulty_mod = None;
            for line in log_text.lines() {
                if let Some(caps) = re_mod.captures(line) {
                    faulty_mod = Some(caps["mod"].to_string());
                    break;
                }
            }

            let evidence = log_text
                .lines()
                .find(|l| l.contains("MixinApplyError") || l.contains("Critical injection failure"))
                .unwrap_or("Mixin dönüşüm hatası")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Mod Kod Çakışması (Mixin Enjeksiyon Hatası)".to_string(),
                severity: CrashSeverity::Critical,
                faulty_mod: faulty_mod.clone(),
                cause: format!(
                    "İki mod aynı oyun kodunu eşzamanlı olarak değiştirmeye çalıştı. Hedef sınıf dönüştürülemedi (Şüpheli Mod: {}).",
                    faulty_mod.as_deref().unwrap_or("Tespit Edilen Mod")
                ),
                evidence,
                recommendation: "Çakışmaya neden olan modu Mod Merkezi sekmesinden geçici olarak devre dışı bırakın veya modun güncel sürümünü yükleyin.".to_string(),
                confidence_score: 0.94,
            });
        }

        // 6. Senaryo: Eksik Mod Bağımlılığı (Fabric/NeoForge/Forge Dependency)
        if log_text.contains("ModResolutionException")
            || log_text.contains("requires") && log_text.contains("which is missing")
            || log_text.contains("Missing or unsupported mandatory dependencies")
        {
            let re = Regex::new(r"Mod '(?P<mod>[^']+)' requires (?P<req>[^,\n]+)").unwrap();
            let mut faulty = None;
            let mut evidence = String::new();

            for line in log_text.lines() {
                if let Some(caps) = re.captures(line) {
                    faulty = Some(caps["mod"].to_string());
                    evidence = line.trim().to_string();
                    break;
                }
            }

            if evidence.is_empty() {
                evidence = log_text
                    .lines()
                    .find(|l| l.contains("requires") || l.contains("dependencies"))
                    .unwrap_or("Eksik bağımlılık hatası")
                    .trim()
                    .to_string();
            }

            return Ok(CrashDiagnosis {
                title: "Eksik Mod Bağımlılığı (Gerekli Kütüphane Yok)".to_string(),
                severity: CrashSeverity::Critical,
                faulty_mod: faulty.clone(),
                cause: format!(
                    "Kurulu modlardan biri ({}) çalışmak için ek bir yardımcı kütüphaneye ihtiyaç duyuyor ancak bu kütüphane profilinizde yüklü değil.",
                    faulty.unwrap_or_else(|| "Mod".to_string())
                ),
                evidence,
                recommendation: "Mod Merkezi sekmesinden eksik bağımlılığı (Örn: Architectury API, Cloth Config, Fabric API vb.) aratıp kurun.".to_string(),
                confidence_score: 0.95,
            });
        }

        // 7. Senaryo: Çift / Yinelenen Mod ID (Duplicate Mods)
        if log_text.contains("DuplicateModsFoundException")
            || log_text.contains("Mod loading error: Duplicate mod")
            || log_text.contains("already exists in the mod list")
        {
            let evidence = log_text
                .lines()
                .find(|l| l.contains("Duplicate") || l.contains("already exists"))
                .unwrap_or("Duplicate mod detected")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Mükerrer / Çift Mod Dosyası".to_string(),
                severity: CrashSeverity::Warning,
                faulty_mod: None,
                cause: "Mods klasöründe aynı modun iki farklı sürümü (veya aynı mod ID'sine sahip iki JAR) mevcut.".to_string(),
                evidence,
                recommendation: "Mod Merkezi -> Kurulu Modlar sekmesine gidip aynı ada sahip eski versiyon JAR dosyasını silin.".to_string(),
                confidence_score: 0.96,
            });
        }

        // 8. Senaryo: Yanlış Mod Yükleyici (Forge modunu Fabric'te veya tersi)
        if log_text.contains("is not a valid Fabric mod")
            || log_text.contains("Invalid mod file found: missing fabric.mod.json")
            || log_text.contains("is missing mods.toml file")
        {
            let evidence = log_text
                .lines()
                .find(|l| l.contains("fabric.mod.json") || l.contains("mods.toml"))
                .unwrap_or("Uyumsuz mod loader formatı")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Mod Yükleyici (Loader) Uyumsuzluğu".to_string(),
                severity: CrashSeverity::Warning,
                faulty_mod: None,
                cause: "Kurulu modlardan biri profilinizin mod yükleyicisiyle uyuşmuyor (Örn: Forge için yapılmış bir mod Fabric profilinde çalıştırılmaya çalışılıyor).".to_string(),
                evidence,
                recommendation: "Profilinizin Forge mu yoksa Fabric mi olduğunu kontrol edin ve yalnızca o yükleyici için derlenmiş mod sürümlerini indirin.".to_string(),
                confidence_score: 0.92,
            });
        }

        // 9. Senaryo: Shader / Oculus / Iris / Embeddium Çökmesi
        if log_text.contains("net.coderbot.iris")
            || log_text.contains("me.jellysquid.mods.sodium")
            || log_text.contains("com.mojang.blaze3d.shaders")
            || log_text.contains("Oculus")
        {
            let evidence = log_text
                .lines()
                .find(|l| l.contains("iris") || l.contains("sodium") || l.contains("Oculus") || l.contains("shaders"))
                .unwrap_or("Shader render motoru hatası")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Shader / Grafik Motoru Çökmesi".to_string(),
                severity: CrashSeverity::Warning,
                faulty_mod: Some("Oculus/Iris/Embeddium".to_string()),
                cause: "Aktif shaderpack veya render optimizasyon modu ekran kartı gölgelendiricilerini derlerken hata verdi.".to_string(),
                evidence,
                recommendation: "Shaderpack sekmesinden mevcut shader'ı değiştirin veya geçici olarak devre dışı bırakın. Embeddium/Oculus sürümlerinizi güncelleyin.".to_string(),
                confidence_score: 0.89,
            });
        }

        // 10. Senaryo: Bozuk Dünya / NBT / Ticking Entity
        if log_text.contains("Ticking entity")
            || log_text.contains("Ticking block entity")
            || log_text.contains("IndexOutOfBoundsException: In chunk")
        {
            let evidence = log_text
                .lines()
                .find(|l| l.contains("Ticking entity") || l.contains("Ticking block entity"))
                .unwrap_or("Entity ticking error")
                .trim()
                .to_string();

            return Ok(CrashDiagnosis {
                title: "Bozuk Varlık / Dünya Kaydı (Ticking Entity)".to_string(),
                severity: CrashSeverity::Warning,
                faulty_mod: None,
                cause: "Dünyadaki bir canavar, blok veya eşya güncellenirken sonsuz döngüye veya bozuk veriye rastladı.".to_string(),
                evidence,
                recommendation: "Profil klasörünüze 'Nerf-Entity-Lag' veya 'BadOptimizations' modu ekleyebilir veya çöken varlığı silmek için NBTEdit kullanabilirsiniz.".to_string(),
                confidence_score: 0.88,
            });
        }

        // 11. Senaryo: Belirli bir moddan kaynaklanan genel Exception
        let re_stack = Regex::new(r"at (net|com|me|org|io)\.(?P<mod>[a-zA-Z0-9_]+)\.").unwrap();
        for line in log_text.lines() {
            if line.trim().starts_with("at ") && !line.contains("minecraft") && !line.contains("mojang") {
                if let Some(caps) = re_stack.captures(line) {
                    let mod_name = &caps["mod"];
                    return Ok(CrashDiagnosis {
                        title: format!("'{}' Modundan Kaynaklı Hata", mod_name),
                        severity: CrashSeverity::Warning,
                        faulty_mod: Some(mod_name.to_string()),
                        cause: format!("Çökme stack trace kaydında '{}' moduna ait kod satırları bulundu.", mod_name),
                        evidence: line.trim().to_string(),
                        recommendation: format!("'{}' modunun kurulu sürümünü kontrol edin, devre dışı bırakmayı veya güncellemeyi deneyin.", mod_name),
                        confidence_score: 0.85,
                    });
                }
            }
        }

        // Bilinmeyen / Genel Durum
        Ok(CrashDiagnosis {
            title: "Genel / İncelenmesi Gereken Çökme".to_string(),
            severity: CrashSeverity::Info,
            faulty_mod: None,
            cause: "Log dosyasında yaygın desenlerle doğrudan eşleşen kritik bir anahtar sözcük bulunamadı.".to_string(),
            evidence: log_text.lines().take(3).collect::<Vec<_>>().join(" "),
            recommendation: "Log metnini inceleyin veya Profil Ayarlarından ayrılan belleği artırıp modları teker teker test edin.".to_string(),
            confidence_score: 0.45,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_out_of_memory() {
        let sample = "Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.world.level.Level.<init>";
        let res = CrashAnalyzer::analyze_log(sample).unwrap();
        assert_eq!(res.severity, CrashSeverity::Critical);
        assert!(res.title.contains("RAM"));
        assert_eq!(res.confidence_score, 0.99);
    }

    #[test]
    fn test_detect_unsupported_class_version() {
        let sample = "java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime";
        let res = CrashAnalyzer::analyze_log(sample).unwrap();
        assert!(res.title.contains("Java Sürümü"));
    }

    #[test]
    fn test_detect_mixin_conflict() {
        let sample = "org.spongepowered.asm.mixin.transformer.throwables.MixinApplyError: Mixin [sodium.mixins.json:features.buffer_builder.MixinBufferBuilder] failed injection";
        let res = CrashAnalyzer::analyze_log(sample).unwrap();
        assert!(res.title.contains("Mixin"));
    }

    #[test]
    fn test_detect_glfw_error() {
        let sample = "GLFW error 65542: WGL: The driver does not appear to support OpenGL";
        let res = CrashAnalyzer::analyze_log(sample).unwrap();
        assert!(res.title.contains("OpenGL"));
    }
}
