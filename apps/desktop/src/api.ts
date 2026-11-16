import { CreateInstanceRequest, InstanceSummary, SystemStatus } from "./types";

export interface VersionManifestEntry {
  id: string;
  type: string;
  url: string;
  releaseTime: string;
}

export interface LaunchResultResponse {
  success: boolean;
  pid?: number;
  message: string;
}

export interface ModSearchResult {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  icon_url?: string;
  downloads: number;
  follows: number;
  categories: string[];
  supported_versions: string[];
  /** Proje galerisi (kapak görselleri) — ilk görsel kart kapağı olarak kullanılır. */
  gallery?: string[];
  /** Sağlayıcının önerdiği vurgu rengi (0xRRGGBB). */
  color?: number;
}

export interface SearchFilterQuery {
  query: string;
  game_version?: string;
  loader?: string;
  category?: string;
  sort_by?: "relevance" | "downloads" | "follows" | "newest" | "updated";
  offset: number;
  limit: number;
  project_type?: "mod" | "modpack" | "shader";
}

export interface SearchPageResult {
  hits: ModSearchResult[];
  total_hits: number;
  offset: number;
  limit: number;
}

export interface InstalledModInfo {
  file_name: string;
  is_enabled: boolean;
  file_size: number;
  sha512_hash?: string;
}

// Tauri IPC çağrısı veya yerel fallback mekanizması
async function invokeTauri<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }

  // Tarayıcı / test önizleme fallback durumu
  console.info(`[Dev Fallback] IPC invoke: ${cmd}`, args);
  return mockHandler<T>(cmd, args);
}

// Geliştirme aşamasında web önizlemesi için mock veritabanı
let mockInstances: InstanceSummary[] = [
  {
    id: "demo-instance-1",
    name: "Survival Odyssey 1.21",
    directory_name: "survival-1.21-a1b2c3d4",
    minecraft_version: "1.21.1",
    loader_type: "fabric",
    loader_version: "0.16.5",
    allocated_memory_mb: 6144,
    created_at: "2026-10-28 20:00:00",
    last_played_at: "Bugün 21:15",
  },
  {
    id: "demo-instance-2",
    name: "Vanilla Speedrun",
    directory_name: "vanilla-speedrun-e5f6g7h8",
    minecraft_version: "1.21.1",
    loader_type: "vanilla",
    allocated_memory_mb: 4096,
    created_at: "2026-10-27 15:30:00",
    last_played_at: "Dün 18:40",
  },
];

let mockInstalledMods: Record<string, InstalledModInfo[]> = {
  "demo-instance-1": [
    {
      file_name: "sodium-fabric-mc1.21.1-0.5.11.jar",
      is_enabled: true,
      file_size: 1240500,
      sha512_hash: "mock512hash",
    },
    {
      file_name: "iris-mc1.21.1-1.7.5.jar",
      is_enabled: true,
      file_size: 2890000,
      sha512_hash: "mock512hash2",
    },
    {
      file_name: "ferritecore-7.0.0-fabric.jar.disabled",
      is_enabled: false,
      file_size: 320100,
      sha512_hash: "mock512hash3",
    },
  ],
};

function mockHandler<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  switch (cmd) {
    case "get_system_status":
      return Promise.resolve({
        version: "0.1.0-alpha (G3)",
        target_os: "windows-x64",
        data_dir: "%LOCALAPPDATA%\\eneslauncher\\EnesLauncher\\data",
        instances_count: mockInstances.length,
        java_path: "C:\\Program Files\\Java\\jdk-17\\bin\\java.exe",
      } as unknown as T);

    case "list_instances":
      return Promise.resolve([...mockInstances] as unknown as T);

    case "create_instance": {
      const req = (args?.request || {}) as CreateInstanceRequest;
      const newInst: InstanceSummary = {
        id: "inst-" + Math.random().toString(36).substring(2, 9),
        name: req.name || "Yeni Profil",
        directory_name: (req.name || "profil").toLowerCase().replace(/[^a-z0-9]/g, "_") + "-auto",
        minecraft_version: req.minecraft_version || "1.21.1",
        loader_type: req.loader_type || "vanilla",
        loader_version: req.loader_version,
        allocated_memory_mb: req.allocated_memory_mb || 4096,
        created_at: new Date().toISOString().replace("T", " ").substring(0, 19),
      };
      mockInstances.unshift(newInst);
      return Promise.resolve(newInst as unknown as T);
    }

    case "delete_instance": {
      const id = args?.id as string;
      mockInstances = mockInstances.filter((i) => i.id !== id);
      return Promise.resolve(true as unknown as T);
    }

    case "test_archive_security": {
      const p = (args?.testPath as string) || "";
      if (p.includes("..") || p.startsWith("/") || p.includes(":") || p.endsWith(".exe") || p.endsWith(".dll")) {
        return Promise.reject(`Güvenlik engeli: Path traversal veya yasaklı dosya uzantısı tespit edildi: '${p}'`);
      }
      return Promise.resolve(`Geçerli ve güvenli yol: C:/games/instance/${p}` as unknown as T);
    }

    case "fetch_official_versions": {
      return Promise.resolve([
        { id: "1.21.1", type: "release", url: "", releaseTime: "2024-08-08" },
        { id: "1.21", type: "release", url: "", releaseTime: "2024-06-13" },
        { id: "1.20.4", type: "release", url: "", releaseTime: "2023-12-07" },
        { id: "1.20.1", type: "release", url: "", releaseTime: "2023-06-12" },
      ] as unknown as T);
    }

    case "search_modrinth_mods": {
      return Promise.resolve([
        {
          project_id: "AANobbMI",
          slug: "sodium",
          title: "Sodium",
          description: "Modern rendering engine drastically improving Minecraft frame rates.",
          author: "jellysquid",
          icon_url: "https://cdn.modrinth.com/data/AANobbMI/icon.png",
          downloads: 42500000,
          follows: 125000,
          categories: ["fabric", "performance"],
          supported_versions: ["1.21.1", "1.20.4"],
        },
        {
          project_id: "YL57xq9U",
          slug: "iris",
          title: "Iris Shaders",
          description: "A modern shaders mod for Minecraft compatible with existing shaders.",
          author: "coderbot",
          icon_url: "https://cdn.modrinth.com/data/YL57xq9U/icon.png",
          downloads: 36000000,
          follows: 98000,
          categories: ["fabric", "shaders"],
          supported_versions: ["1.21.1", "1.20.4"],
        },
      ] as unknown as T);
    }

    case "list_instance_mods": {
      const instId = (args?.instanceId as string) || "demo-instance-1";
      return Promise.resolve((mockInstalledMods[instId] || []) as unknown as T);
    }

    case "toggle_instance_mod": {
      const fileName = args?.fileName as string;
      return Promise.resolve(
        (fileName.endsWith(".disabled")
          ? fileName.replace(".disabled", "")
          : fileName + ".disabled") as unknown as T
      );
    }

    case "delete_instance_mod": {
      return Promise.resolve(true as unknown as T);
    }

    case "import_mrpack": {
      const filePath = (args?.filePath as string) || "sample.mrpack";
      const baseName = filePath.split(/[\\/]/).pop()?.replace(".mrpack", "") || "Modpack Instance";
      const newInst: InstanceSummary = {
        id: "mrpack-" + Math.random().toString(36).substring(2, 9),
        name: baseName,
        directory_name: baseName.toLowerCase().replace(/[^a-z0-9]/g, "_") + "-pack",
        minecraft_version: "1.21.1",
        loader_type: "fabric",
        loader_version: "0.16.5",
        allocated_memory_mb: 6144,
        created_at: new Date().toISOString().replace("T", " ").substring(0, 19),
      };
      mockInstances.unshift(newInst);
      return Promise.resolve(newInst as unknown as T);
    }

    case "export_instance_to_mrpack": {
      return Promise.resolve(true as unknown as T);
    }

    case "open_in_browser":
    case "open_instance_folder": {
      return Promise.resolve(undefined as unknown as T);
    }

    case "update_instance_settings": {
      return Promise.resolve(true as unknown as T);
    }

    case "optimize_instance_performance": {
      return Promise.resolve({
        success: true,
        steps_completed: [
          "options.txt grafikleri Hızlı (Fast) moduna çekildi.",
          "Görüş mesafesi 8 Chunk olarak optimize edildi.",
          "Sodium / Iris GPU hızlandırması etkinleştirildi.",
          "Aikar's G1GC düşük gecikmeli bellek argümanları uygulandı."
        ],
        details: ["Simülasyon 6 chunk, VSync kapalı"],
        estimated_fps_boost: "+%40 - +%85 FPS Artışı",
      } as unknown as T);
    }

    case "install_remote_mod":
    case "install_modrinth_mod": {
      return Promise.resolve(true as unknown as T);
    }

    case "launch_instance": {
      return Promise.resolve({
        success: true,
        pid: 14592,
        message: "Minecraft 1.21.1 (Fabric) başarıyla başlatıldı! (PID: 14592)",
      } as unknown as T);
    }

    case "analyze_crash_log": {
      const content = (args?.logContent as string) || "";
      // Deterministik mock — gerçek Rust logic G5 crate'inde
      if (content.toLowerCase().includes("outofmemoryerror")) {
        return Promise.resolve({
          title: "Yetersiz Bellek — JVM Çöktü",
          severity: "critical",
          faulty_mod: null,
          cause: "JVM yığın alanı doldu. Mevcut RAM tahsisi yetmiyor.",
          evidence: "java.lang.OutOfMemoryError: Java heap space",
          recommendation: "Launcher'da ayrılan RAM'i artırın (≥ 6 GB önerilir). -Xmx ve -Xms değerlerini kontrol edin.",
          confidence_score: 0.97,
        } as unknown as T);
      }
      if (content.toLowerCase().includes("mixinapplyerror") || content.toLowerCase().includes("mixin")) {
        const modMatch = content.match(/\[(.+?)\]/);
        return Promise.resolve({
          title: "Mixin Uygulama Hatası",
          severity: "high",
          faulty_mod: modMatch ? modMatch[1] : "Bilinmeyen Mod",
          cause: "Bir mod, hedef sınıfı Mixin ile yamalamaya çalıştı; sınıf uyumsuz.",
          evidence: "MixinApplyError: Mixin application failed",
          recommendation: "Mixin hatasına yol açan modu güncelleyin veya kaldırın. Loader versiyonunun mevcut MC sürümüyle uyumlu olduğunu doğrulayın.",
          confidence_score: 0.88,
        } as unknown as T);
      }
      if (content.toLowerCase().includes("unsupportedclassversionerror")) {
        return Promise.resolve({
          title: "Yanlış Java Sürümü",
          severity: "high",
          faulty_mod: null,
          cause: "Mod veya oyun dosyası, mevcut JRE'den daha yeni bir Java sürümü gerektiriyor.",
          evidence: "java.lang.UnsupportedClassVersionError",
          recommendation: "Launcher'da Java 21 veya daha yeni bir JDK seçin. Java Runtime Yöneticisi'nden güncelleyin.",
          confidence_score: 0.95,
        } as unknown as T);
      }
      // Genel fallback
      return Promise.resolve({
        title: "Crash Log Analiz Edildi",
        severity: "medium",
        faulty_mod: null,
        cause: "Tanınmayan crash türü. Log detaylı inceleme gerektiriyor.",
        evidence: content.substring(0, 200),
        recommendation: "Tam crash log'u geliştiricilere gönderin veya Modrinth sorun izleyicisinde arayın.",
        confidence_score: 0.45,
      } as unknown as T);
    }

    case "search_modrinth_advanced": {
      const filter = (args?.filter || {}) as SearchFilterQuery;
      if (filter.project_type === "modpack") {
        return Promise.resolve({
          hits: [
            {
              project_id: "1KVoqANr",
              slug: "fabulously-optimized",
              title: "Fabulously Optimized",
              description: "Simple Minecraft performance modpack with OptiFine parity shaders and high FPS.",
              author: "RobotKoer",
              icon_url: "https://cdn.modrinth.com/data/1KVoqANr/icon.png",
              downloads: 7200000,
              follows: 85000,
              categories: ["fabric", "optimization"],
              supported_versions: ["1.21.1", "1.20.4", "1.19.4"],
            },
            {
              project_id: "u6d4WMR2",
              slug: "simply-optimized",
              title: "Simply Optimized",
              description: "Performance-focused modpack providing minimal tweaks for pure speed.",
              author: "Moulberry",
              icon_url: "https://cdn.modrinth.com/data/u6d4WMR2/icon.png",
              downloads: 4100000,
              follows: 42000,
              categories: ["fabric", "performance"],
              supported_versions: ["1.21.1", "1.20.4"],
            },
            {
              project_id: "N6U6D6hB",
              slug: "all-of-fabric-7",
              title: "All of Fabric 7",
              description: "Comprehensive Fabric modpack featuring tech, magic, exploration, and building.",
              author: "TeamAOF",
              icon_url: "https://cdn.modrinth.com/data/N6U6D6hB/icon.png",
              downloads: 2900000,
              follows: 31000,
              categories: ["fabric", "adventure", "technology"],
              supported_versions: ["1.20.1"],
            },
          ],
          total_hits: 156,
          offset: filter.offset || 0,
          limit: filter.limit || 10,
        } as unknown as T);
      }

      return Promise.resolve({
        hits: [
          {
            project_id: "AANobbMI",
            slug: "sodium",
            title: "Sodium",
            description: "Modern rendering engine drastically improving Minecraft frame rates.",
            author: "jellysquid",
            icon_url: "https://cdn.modrinth.com/data/AANobbMI/icon.png",
            downloads: 42500000,
            follows: 125000,
            categories: ["fabric", "performance"],
            supported_versions: ["1.21.1", "1.20.4"],
          },
          {
            project_id: "YL57xq9U",
            slug: "iris",
            title: "Iris Shaders",
            description: "A modern shaders mod for Minecraft compatible with existing shaders.",
            author: "coderbot",
            icon_url: "https://cdn.modrinth.com/data/YL57xq9U/icon.png",
            downloads: 36000000,
            follows: 98000,
            categories: ["fabric", "shaders"],
            supported_versions: ["1.21.1", "1.20.4"],
          },
        ],
        total_hits: 240,
        offset: 0,
        limit: 12,
      } as unknown as T);
    }

    case "search_modrinth_modpacks": {
      return Promise.resolve([
        {
          project_id: "1KVoqANr",
          slug: "fabulously-optimized",
          title: "Fabulously Optimized",
          description: "Simple Minecraft performance modpack with OptiFine parity shaders and high FPS.",
          author: "RobotKoer",
          icon_url: "https://cdn.modrinth.com/data/1KVoqANr/icon.png",
          downloads: 7200000,
          follows: 85000,
          categories: ["fabric", "optimization"],
          supported_versions: ["1.21.1", "1.20.4", "1.19.4"],
        },
        {
          project_id: "u6d4WMR2",
          slug: "simply-optimized",
          title: "Simply Optimized",
          description: "Performance-focused modpack providing minimal tweaks for pure speed.",
          author: "Moulberry",
          icon_url: "https://cdn.modrinth.com/data/u6d4WMR2/icon.png",
          downloads: 4100000,
          follows: 42000,
          categories: ["fabric", "performance"],
          supported_versions: ["1.21.1", "1.20.4"],
        },
        {
          project_id: "N6U6D6hB",
          slug: "all-of-fabric-7",
          title: "All of Fabric 7",
          description: "Comprehensive Fabric modpack featuring tech, magic, exploration, and building.",
          author: "TeamAOF",
          icon_url: "https://cdn.modrinth.com/data/N6U6D6hB/icon.png",
          downloads: 2900000,
          follows: 31000,
          categories: ["fabric", "adventure", "technology"],
          supported_versions: ["1.20.1"],
        },
      ] as unknown as T);
    }

    case "get_modpack_versions": {
      return Promise.resolve([
        {
          id: "v-5.8.0",
          project_id: "1KVoqANr",
          name: "v5.8.0 for 1.21.1",
          version_number: "5.8.0",
          game_versions: ["1.21.1"],
          loaders: ["fabric"],
          files: [
            {
              filename: "Fabulously.Optimized-5.8.0.mrpack",
              url: "https://cdn.modrinth.com/data/1KVoqANr/versions/v-5.8.0/FO-5.8.0.mrpack",
              size: 2450000,
              primary: true,
            },
          ],
        },
        {
          id: "v-5.7.0",
          project_id: "1KVoqANr",
          name: "v5.7.0 for 1.20.4",
          version_number: "5.7.0",
          game_versions: ["1.20.4"],
          loaders: ["fabric"],
          files: [
            {
              filename: "Fabulously.Optimized-5.7.0.mrpack",
              url: "https://cdn.modrinth.com/data/1KVoqANr/versions/v-5.7.0/FO-5.7.0.mrpack",
              size: 2310000,
              primary: true,
            },
          ],
        },
      ] as unknown as T);
    }

    case "install_remote_modpack": {
      const packName = (args?.packName as string) || "Modpack Instance";
      const newInst: InstanceSummary = {
        id: "pack-" + Math.random().toString(36).substring(2, 9),
        name: packName,
        directory_name: packName.toLowerCase().replace(/[^a-z0-9]/g, "_") + "-modpack",
        minecraft_version: "1.21.1",
        loader_type: "fabric",
        loader_version: "0.16.5",
        allocated_memory_mb: 6144,
        created_at: new Date().toISOString().replace("T", " ").substring(0, 19),
      };
      mockInstances.unshift(newInst);
      return Promise.resolve(newInst as unknown as T);
    }

    case "list_accounts": {
      return Promise.resolve([
        {
          id: "default-offline",
          username: "Enes",
          uuid: "c53e291d-06cf-4768-8fc3-f8fed821af32",
          account_type: "offline",
          is_active: true,
        },
      ] as unknown as T);
    }

    case "add_offline_account": {
      const uname = (args?.username as string) || "YeniOyuncu";
      const acc = {
        id: "offline-" + Math.random().toString(36).substring(2, 8),
        username: uname,
        uuid: "offline-" + Math.random().toString(36).substring(2, 10),
        account_type: "offline",
        is_active: true,
      };
      return Promise.resolve(acc as unknown as T);
    }

    case "set_active_account": {
      return Promise.resolve(true as unknown as T);
    }

    case "delete_account": {
      return Promise.resolve(true as unknown as T);
    }

    case "start_microsoft_login": {
      return Promise.resolve({
        user_code: "MC-7892",
        device_code: "dev_code_mock",
        verification_uri: "https://microsoft.com/link",
        message: "Tarayıcınızda https://microsoft.com/link adresine gidip MC-7892 kodunu girin.",
        interval: 5,
      } as unknown as T);
    }

    case "save_launcher_settings": {
      if (typeof window !== "undefined" && args?.settingsJson) {
        localStorage.setItem("enes_launcher_settings_disk", args.settingsJson as string);
      }
      return Promise.resolve(true as unknown as T);
    }

    case "load_launcher_settings": {
      if (typeof window !== "undefined") {
        const val = localStorage.getItem("enes_launcher_settings_disk");
        return Promise.resolve(val as unknown as T);
      }
      return Promise.resolve(null as unknown as T);
    }

    case "get_file_data_url": {
      return Promise.resolve("" as unknown as T);
    }

    default:
      return Promise.reject(`Bilinmeyen Tauri komutu: ${cmd}`);
  }
}

// ── G5: Crash Analyzer & Auth Service ──────────────────────────────────────

export interface AccountProfile {
  id: string;
  username: string;
  uuid: string;
  account_type: "offline" | "microsoft";
  is_active: boolean;
  skin_url?: string;
}

export interface DeviceCodeResponse {
  user_code: string;
  device_code: string;
  verification_uri: string;
  message: string;
  interval: number;
}

export interface ModpackVersionInfo {
  id: string;
  project_id: string;
  name: string;
  version_number: string;
  game_versions: string[];
  loaders: string[];
  date_published?: string;
  files: {
    filename: string;
    url: string;
    size: number;
    primary: boolean;
  }[];
}

export interface CrashDiagnosis {
  title: string;
  severity: "critical" | "high" | "medium" | "low" | "info";
  faulty_mod: string | null;
  cause: string;
  evidence: string;
  recommendation: string;
  confidence_score: number; // 0.0–1.0
}

const versionCache = new Map<string, { timestamp: number; data: ModpackVersionInfo[] }>();
const searchCache = new Map<string, { timestamp: number; data: SearchPageResult }>();

export interface OptimizationReport {
  success: boolean;
  steps_completed: string[];
  details: string[];
  estimated_fps_boost: string;
}

export const api = {
  getSystemStatus: () => invokeTauri<SystemStatus>("get_system_status"),
  listInstances: () => invokeTauri<InstanceSummary[]>("list_instances"),
  createInstance: (request: CreateInstanceRequest) =>
    invokeTauri<InstanceSummary>("create_instance", { request }),
  deleteInstance: (id: string) => invokeTauri<boolean>("delete_instance", { id }),
  testArchiveSecurity: (testPath: string) =>
    invokeTauri<string>("test_archive_security", { testPath }),
  fetchOfficialVersions: () =>
    invokeTauri<VersionManifestEntry[]>("fetch_official_versions"),
  launchInstance: (instanceId: string, username: string) =>
    invokeTauri<LaunchResultResponse>("launch_instance", { instanceId, username }),
  searchModrinthAdvanced: async (filter: SearchFilterQuery) => {
    const key = JSON.stringify(filter);
    const cached = searchCache.get(key);
    if (cached && Date.now() - cached.timestamp < 120_000) {
      return cached.data;
    }
    const res = await invokeTauri<SearchPageResult>("search_modrinth_advanced", { filter });
    searchCache.set(key, { timestamp: Date.now(), data: res });
    return res;
  },
  searchModrinthMods: (query: string, gameVersion?: string, loader?: string) =>
    invokeTauri<ModSearchResult[]>("search_modrinth_mods", { query, gameVersion, loader }),
  listInstanceMods: (instanceId: string) =>
    invokeTauri<InstalledModInfo[]>("list_instance_mods", { instanceId }),
  toggleInstanceMod: (instanceId: string, fileName: string) =>
    invokeTauri<string>("toggle_instance_mod", { instanceId, fileName }),
  deleteInstanceMod: (instanceId: string, fileName: string) =>
    invokeTauri<boolean>("delete_instance_mod", { instanceId, fileName }),
  installRemoteMod: (instanceId: string, downloadUrl: string, fileName: string) =>
    invokeTauri<boolean>("install_remote_mod", { instanceId, downloadUrl, fileName }),
  installModrinthMod: (instanceId: string, versionId: string) =>
    invokeTauri<string>("install_modrinth_mod", { instanceId, versionId }),
  importMrpack: (filePath: string, customName?: string) =>
    invokeTauri<InstanceSummary>("import_mrpack", { filePath, customName }),
  exportInstanceToMrpack: (instanceId: string, exportPath: string) =>
    invokeTauri<boolean>("export_instance_to_mrpack", { instanceId, exportPath }),

  // G5: AI Crash Analizi
  analyzeCrashLog: (logContent: string) =>
    invokeTauri<CrashDiagnosis>("analyze_crash_log", { logContent }),
  getInstanceLatestLog: (instanceId: string) =>
    invokeTauri<string>("get_instance_latest_log", { instanceId }),
  setInstanceLanguage: (instanceId: string, lang: string) =>
    invokeTauri<boolean>("set_instance_language", { instanceId, lang }),

  // Shaderpack Yönetimi
  listInstanceShaders: (instanceId: string) =>
    invokeTauri<string[]>("list_instance_shaders", { instanceId }),
  installRemoteShader: (instanceId: string, downloadUrl: string, fileName: string) =>
    invokeTauri<boolean>("install_remote_shader", { instanceId, downloadUrl, fileName }),
  deleteInstanceShader: (instanceId: string, fileName: string) =>
    invokeTauri<boolean>("delete_instance_shader", { instanceId, fileName }),

  // Özel Arka Plan ve Sistem
  saveCustomBackground: (fileName: string, dataBase64: string) =>
    invokeTauri<string>("save_custom_background", { fileName, dataBase64 }),
  saveLauncherSettings: (settingsJson: string) =>
    invokeTauri<boolean>("save_launcher_settings", { settingsJson }),
  loadLauncherSettings: () =>
    invokeTauri<string | null>("load_launcher_settings"),
  getFileDataUrl: (filePath: string) =>
    invokeTauri<string>("get_file_data_url", { filePath }),
  openInBrowser: (url: string) =>
    invokeTauri<void>("open_in_browser", { url }),
  openInstanceFolder: (instanceId: string) =>
    invokeTauri<void>("open_instance_folder", { instanceId }),
  updateInstanceSettings: (instanceId: string, name?: string, memoryMb?: number, jvmArguments?: string) =>
    invokeTauri<boolean>("update_instance_settings", { instanceId, name, memoryMb, jvmArguments }),
  optimizeInstancePerformance: (instanceId: string) =>
    invokeTauri<OptimizationReport>("optimize_instance_performance", { instanceId }),

  // Modpack Keşfi & Tek Tıkla Kurulum
  searchModrinthModpacks: (query: string, gameVersion?: string, loader?: string) =>
    invokeTauri<ModSearchResult[]>("search_modrinth_modpacks", { query, gameVersion, loader }),
  getModpackVersions: async (projectId: string, gameVersion?: string, loader?: string) => {
    const key = `${projectId}:${gameVersion || ""}:${loader || ""}`;
    const cached = versionCache.get(key);
    if (cached && Date.now() - cached.timestamp < 300_000) {
      return cached.data;
    }
    const res = await invokeTauri<ModpackVersionInfo[]>("get_modpack_versions", { projectId, gameVersion, loader });
    versionCache.set(key, { timestamp: Date.now(), data: res });
    return res;
  },
  installRemoteModpack: (
    downloadUrl: string,
    packName: string,
    iconUrl?: string,
    coverUrl?: string,
    projectId?: string,
  ) =>
    invokeTauri<InstanceSummary>("install_remote_modpack", {
      downloadUrl,
      packName,
      iconUrl: iconUrl ?? null,
      coverUrl: coverUrl ?? null,
      projectId: projectId ?? null,
    }),

  /** Görseli olmayan mevcut profil için modpack kapağını tamamlar. */
  backfillInstanceArt: (instanceId: string) =>
    invokeTauri<InstanceSummary | null>("backfill_instance_art", { instanceId }),

  // Hesap Yönetimi (Çevrimdışı & Çevrimiçi)
  listAccounts: () => invokeTauri<AccountProfile[]>("list_accounts"),
  addOfflineAccount: (username: string) =>
    invokeTauri<AccountProfile>("add_offline_account", { username }),
  setActiveAccount: (accountId: string) =>
    invokeTauri<AccountProfile>("set_active_account", { accountId }),
  deleteAccount: (accountId: string) =>
    invokeTauri<boolean>("delete_account", { accountId }),
  startMicrosoftLogin: () =>
    invokeTauri<DeviceCodeResponse>("start_microsoft_login"),
};
