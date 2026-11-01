import { useEffect, useRef } from "react";

// Tauri WebView'da yerel dosya yollarını ve Data/Blob URL'leri güvenli URL'ye çevirir
export function toDisplayUrl(path: string): string {
  if (!path || path.trim() === "" || path.includes("[indexeddb]")) return "";
  if (path.startsWith("data:") || path.startsWith("blob:")) return path;
  if (path.startsWith("http://") || path.startsWith("https://")) return path;
  if (path.startsWith("asset://") || path.startsWith("tauri://")) return path;

  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    try {
      const convert = (window as any).__TAURI_INTERNALS__?.convertFileSrc;
      if (typeof convert === "function") {
        return convert(path);
      }
    } catch (_) {}
  }

  return "file:///" + path.replace(/\\/g, "/");
}

export type ThemePreset =
  | "warden"
  | "amethyst"
  | "emerald"
  | "crimson"
  | "ocean"
  | "ender";

export interface BackgroundSettings {
  type: "particles" | "custom_image" | "custom_video";
  customImageUrl: string;
  customVideoUrl: string;
  customImagePath?: string;
  customVideoPath?: string;
  presetTheme: ThemePreset;
  blurAmount: number; // 0 - 20 px
  opacity: number; // 0.1 - 1.0
  particlesEnabled: boolean;
}

export const defaultBgSettings: BackgroundSettings = {
  type: "particles",
  customImageUrl: "",
  customVideoUrl: "",
  customImagePath: "",
  customVideoPath: "",
  presetTheme: "warden",
  blurAmount: 0,
  opacity: 0.85,
  particlesEnabled: true,
};

export const themeStyles: Record<
  ThemePreset,
  {
    name: string;
    primary: string;
    secondary: string;
    accentGlow: string;
    borderAccent: string;
    badgeBg: string;
    bgGradient: string;
    particleColors: string[];
  }
> = {
  warden: {
    name: "Warden & Deep Dark (Sculk Turkuaz)",
    primary: "from-teal-400 via-cyan-400 to-sky-500",
    secondary: "text-cyan-400",
    accentGlow: "rgba(0, 245, 212, 0.3)",
    borderAccent: "border-cyan-500/40 hover:border-teal-400/60",
    badgeBg: "bg-cyan-950/60 text-cyan-300 border-cyan-500/30",
    bgGradient: "radial-gradient(ellipse at 50% 0%, #031e28 0%, #020c13 50%, #010408 100%)",
    particleColors: ["#00f5d4", "#38bdf8", "#0284c7", "#5eead4"],
  },
  amethyst: {
    name: "Amethyst Cyber (Neon Mor & Camgöbeği)",
    primary: "from-purple-500 via-fuchsia-500 to-cyan-400",
    secondary: "text-purple-400",
    accentGlow: "rgba(168, 85, 247, 0.25)",
    borderAccent: "border-purple-500/40 hover:border-cyan-400/60",
    badgeBg: "bg-purple-950/60 text-purple-300 border-purple-500/30",
    bgGradient: "radial-gradient(ellipse at 50% 0%, #1a0b2e 0%, #0d0718 50%, #06030a 100%)",
    particleColors: ["#c084fc", "#e879f9", "#38bdf8", "#818cf8"],
  },
  emerald: {
    name: "Emerald Obsidian (Klasik Zümrüt)",
    primary: "from-emerald-500 via-teal-400 to-cyan-300",
    secondary: "text-emerald-400",
    accentGlow: "rgba(16, 185, 129, 0.25)",
    borderAccent: "border-emerald-500/40 hover:border-teal-400/60",
    badgeBg: "bg-emerald-950/60 text-emerald-300 border-emerald-500/30",
    bgGradient: "radial-gradient(ellipse at 50% 0%, #06241b 0%, #061512 50%, #030807 100%)",
    particleColors: ["#34d399", "#10b981", "#2dd4bf", "#6ee7b7"],
  },
  crimson: {
    name: "Crimson Nether (Ateş & Yakut)",
    primary: "from-rose-500 via-red-500 to-amber-400",
    secondary: "text-rose-400",
    accentGlow: "rgba(244, 63, 94, 0.25)",
    borderAccent: "border-rose-500/40 hover:border-amber-400/60",
    badgeBg: "bg-rose-950/60 text-rose-300 border-rose-500/30",
    bgGradient: "radial-gradient(ellipse at 50% 0%, #2b0b0e 0%, #150608 50%, #080203 100%)",
    particleColors: ["#fb7185", "#f43f5e", "#f59e0b", "#fda4af"],
  },
  ocean: {
    name: "Prismarine Ocean (Okyanus & Turkuaz)",
    primary: "from-sky-500 via-blue-500 to-teal-400",
    secondary: "text-sky-400",
    accentGlow: "rgba(14, 165, 233, 0.25)",
    borderAccent: "border-sky-500/40 hover:border-teal-400/60",
    badgeBg: "bg-sky-950/60 text-sky-300 border-sky-500/30",
    bgGradient: "radial-gradient(ellipse at 50% 0%, #082138 0%, #051321 50%, #02070c 100%)",
    particleColors: ["#38bdf8", "#0ea5e9", "#2dd4bf", "#7dd3fc"],
  },
  ender: {
    name: "End Void & Dragon (Ejderha Moru)",
    primary: "from-fuchsia-500 via-purple-600 to-pink-500",
    secondary: "text-fuchsia-400",
    accentGlow: "rgba(217, 70, 239, 0.3)",
    borderAccent: "border-fuchsia-500/40 hover:border-pink-400/60",
    badgeBg: "bg-fuchsia-950/60 text-fuchsia-300 border-fuchsia-500/30",
    bgGradient: "radial-gradient(ellipse at 50% 0%, #220326 0%, #100213 50%, #040005 100%)",
    particleColors: ["#e879f9", "#d946ef", "#c026d3", "#f0abfc"],
  },
};

export function BackgroundCanvas({ settings }: { settings: BackgroundSettings }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const currentTheme = themeStyles[settings.presetTheme] || themeStyles.warden;

  useEffect(() => {
    // Body'nin data-theme özelliğini güncelle -> İmleç rengi ve teması anında güncellenir!
    document.body.setAttribute("data-theme", settings.presetTheme);
  }, [settings.presetTheme]);

  useEffect(() => {
    if (!settings.particlesEnabled) return;
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let animId: number;
    let width = (canvas.width = window.innerWidth);
    let height = (canvas.height = window.innerHeight);

    const handleResize = () => {
      if (!canvas) return;
      width = canvas.width = window.innerWidth;
      height = canvas.height = window.innerHeight;
    };
    window.addEventListener("resize", handleResize);

    // Minecraft tarzı süzülen partiküller (Portal orbs & Sculk ruhları)
    const count = 50;
    const particles = Array.from({ length: count }, () => ({
      x: Math.random() * width,
      y: Math.random() * height,
      size: Math.random() * 4.5 + 1.5,
      speedX: (Math.random() - 0.5) * 0.4,
      speedY: -Math.random() * 0.8 - 0.2, // yukarı süzülme
      opacity: Math.random() * 0.6 + 0.2,
      color:
        currentTheme.particleColors[
        Math.floor(Math.random() * currentTheme.particleColors.length)
        ],
      rotation: Math.random() * Math.PI,
      rotSpeed: (Math.random() - 0.5) * 0.02,
    }));

    const render = () => {
      ctx.clearRect(0, 0, width, height);

      for (const p of particles) {
        p.x += p.speedX;
        p.y += p.speedY;
        p.rotation += p.rotSpeed;

        if (p.y < -10) {
          p.y = height + 10;
          p.x = Math.random() * width;
        }
        if (p.x < -10) p.x = width + 10;
        if (p.x > width + 10) p.x = -10;

        ctx.save();
        ctx.translate(p.x, p.y);
        ctx.rotate(p.rotation);
        ctx.globalAlpha = p.opacity;
        ctx.fillStyle = p.color;

        // Pikselli kare blok efekti
        ctx.fillRect(-p.size / 2, -p.size / 2, p.size, p.size);
        ctx.restore();
      }

      animId = requestAnimationFrame(render);
    };

    render();

    return () => {
      window.removeEventListener("resize", handleResize);
      cancelAnimationFrame(animId);
    };
  }, [settings.particlesEnabled, settings.presetTheme]);

  return (
    <div className="fixed inset-0 pointer-events-none z-0 overflow-hidden select-none">
      {/* 1. Temel Renk Degradesi (Her zaman en arkada) */}
      <div
        className="absolute inset-0 w-full h-full transition-all duration-700"
        style={{
          background: currentTheme.bgGradient,
        }}
      />

      {/* 2. Özel Video Arka Plan */}
      {settings.type === "custom_video" && settings.customVideoUrl && (
        <video
          autoPlay
          loop
          muted
          playsInline
          className="absolute inset-0 w-full h-full object-cover transition-opacity duration-700"
          style={{
            filter: `blur(${settings.blurAmount}px)`,
            opacity: settings.opacity,
          }}
          src={toDisplayUrl(settings.customVideoUrl)}
        />
      )}

      {/* 3. Özel Fotoğraf / Wallpaper */}
      {settings.type === "custom_image" && settings.customImageUrl && (
        <img
          src={toDisplayUrl(settings.customImageUrl)}
          alt="Arka Plan"
          className="absolute inset-0 w-full h-full object-cover transition-opacity duration-700 pointer-events-none"
          style={{
            filter: `blur(${settings.blurAmount}px)`,
            opacity: settings.opacity,
          }}
          onError={(e) => {
            console.warn("Arka plan görseli yüklenemedi:", settings.customImageUrl);
            // Fallback: e.currentTarget'i gizle
            (e.currentTarget as HTMLElement).style.display = "none";
          }}
        />
      )}

      {/* 4. Canvas Parçacıkları (Magical Minecraft Dust) */}
      {settings.particlesEnabled && (
        <canvas ref={canvasRef} className="absolute inset-0 w-full h-full z-10" />
      )}

      {/* 5. Üst Karartma / Vignette Katmanı */}
      <div className="absolute inset-0 bg-gradient-to-t from-[#020617]/70 via-transparent to-[#020617]/40 pointer-events-none z-10" />
    </div>
  );
}
