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
    name: "Warden",
    primary: "from-teal-400 to-teal-300",
    secondary: "text-teal-300",
    accentGlow: "rgba(45, 212, 191, 0.16)",
    borderAccent: "border-teal-400/30",
    badgeBg: "bg-teal-400/10 text-teal-300 border-teal-400/25",
    bgGradient:
      "radial-gradient(1100px 520px at 50% -12%, rgba(45,212,191,0.09), transparent 62%)",
    particleColors: ["#2dd4bf", "#5eead4", "#14b8a6"],
  },
  amethyst: {
    name: "Amethyst",
    primary: "from-violet-400 to-violet-300",
    secondary: "text-violet-300",
    accentGlow: "rgba(167, 139, 250, 0.16)",
    borderAccent: "border-violet-400/30",
    badgeBg: "bg-violet-400/10 text-violet-300 border-violet-400/25",
    bgGradient:
      "radial-gradient(1100px 520px at 50% -12%, rgba(167,139,250,0.09), transparent 62%)",
    particleColors: ["#a78bfa", "#c4b5fd", "#8b5cf6"],
  },
  emerald: {
    name: "Emerald",
    primary: "from-emerald-400 to-emerald-300",
    secondary: "text-emerald-300",
    accentGlow: "rgba(52, 211, 153, 0.16)",
    borderAccent: "border-emerald-400/30",
    badgeBg: "bg-emerald-400/10 text-emerald-300 border-emerald-400/25",
    bgGradient:
      "radial-gradient(1100px 520px at 50% -12%, rgba(52,211,153,0.09), transparent 62%)",
    particleColors: ["#34d399", "#6ee7b7", "#10b981"],
  },
  crimson: {
    name: "Crimson",
    primary: "from-rose-400 to-rose-300",
    secondary: "text-rose-300",
    accentGlow: "rgba(251, 113, 133, 0.16)",
    borderAccent: "border-rose-400/30",
    badgeBg: "bg-rose-400/10 text-rose-300 border-rose-400/25",
    bgGradient:
      "radial-gradient(1100px 520px at 50% -12%, rgba(251,113,133,0.09), transparent 62%)",
    particleColors: ["#fb7185", "#fda4af", "#f43f5e"],
  },
  ocean: {
    name: "Ocean",
    primary: "from-sky-400 to-sky-300",
    secondary: "text-sky-300",
    accentGlow: "rgba(56, 189, 248, 0.16)",
    borderAccent: "border-sky-400/30",
    badgeBg: "bg-sky-400/10 text-sky-300 border-sky-400/25",
    bgGradient:
      "radial-gradient(1100px 520px at 50% -12%, rgba(56,189,248,0.09), transparent 62%)",
    particleColors: ["#38bdf8", "#7dd3fc", "#0ea5e9"],
  },
  ender: {
    name: "Ender",
    primary: "from-purple-400 to-fuchsia-300",
    secondary: "text-purple-300",
    accentGlow: "rgba(192, 132, 252, 0.16)",
    borderAccent: "border-purple-400/30",
    badgeBg: "bg-purple-400/10 text-purple-300 border-purple-400/25",
    bgGradient:
      "radial-gradient(1100px 520px at 50% -12%, rgba(192,132,252,0.09), transparent 62%)",
    particleColors: ["#c084fc", "#e879f9", "#a855f7"],
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

    // PERF: 60 fps yerine 30 fps. Pencere arkadayken/ikonikken döngü tamamen durur;
    // aksi halde launcher kullanılmazken bile her karede 50 partikül çiziliyordu.
    const FRAME_MS = 1000 / 30;

    let animId = 0;
    let width = (canvas.width = window.innerWidth);
    let height = (canvas.height = window.innerHeight);
    let lastTime = 0;
    let running = !document.hidden;

    const handleResize = () => {
      if (!canvas) return;
      width = canvas.width = window.innerWidth;
      height = canvas.height = window.innerHeight;
    };
    window.addEventListener("resize", handleResize);

    // Sekme/pencere gizlendiğinde animasyonu durdur (CPU/GPU ve pil tasarrufu)
    const handleVisibility = () => {
      running = !document.hidden;
      lastTime = 0;
    };
    document.addEventListener("visibilitychange", handleVisibility);

    // Minecraft tarzı süzülen partiküller (Portal orbs & Sculk ruhları)
    const count = 22;
    const particles = Array.from({ length: count }, () => ({
      x: Math.random() * width,
      y: Math.random() * height,
      size: Math.random() * 2.6 + 1.2,
      speedX: (Math.random() - 0.5) * 0.4,
      speedY: -Math.random() * 0.45 - 0.12, // yukarı süzülme
      opacity: Math.random() * 0.22 + 0.1,
      color:
        currentTheme.particleColors[
        Math.floor(Math.random() * currentTheme.particleColors.length)
        ],
      rotation: Math.random() * Math.PI,
      rotSpeed: (Math.random() - 0.5) * 0.02,
    }));

    const render = (time: number) => {
      animId = requestAnimationFrame(render);

      if (!running) return;
      if (time - lastTime < FRAME_MS) return;

      // Kare atlandığında hız aynı kalsın diye geçen süreye göre ölçekle
      const dt = lastTime === 0 ? 1 : Math.min((time - lastTime) / (1000 / 60), 3);
      lastTime = time;

      ctx.clearRect(0, 0, width, height);

      for (const p of particles) {
        p.x += p.speedX * dt;
        p.y += p.speedY * dt;
        p.rotation += p.rotSpeed * dt;

        if (p.y < -10) {
          p.y = height + 10;
          p.x = Math.random() * width;
        }
        if (p.x < -10) p.x = width + 10;
        if (p.x > width + 10) p.x = -10;

        // save()/restore() yerine tek transform: kare başına 50 çift çağrı azalır
        ctx.setTransform(1, 0, 0, 1, p.x, p.y);
        ctx.rotate(p.rotation);
        ctx.globalAlpha = p.opacity;
        ctx.fillStyle = p.color;

        // Pikselli kare blok efekti
        ctx.fillRect(-p.size / 2, -p.size / 2, p.size, p.size);
      }

      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.globalAlpha = 1;
    };

    animId = requestAnimationFrame(render);

    return () => {
      window.removeEventListener("resize", handleResize);
      document.removeEventListener("visibilitychange", handleVisibility);
      cancelAnimationFrame(animId);
    };
  }, [settings.particlesEnabled, settings.presetTheme]);

  return (
    <div className="fixed inset-0 pointer-events-none z-0 overflow-hidden select-none">
      {/* 1. Temel Renk Degradesi (Her zaman en arkada) */}
      <div
        className="absolute inset-0 w-full h-full transition-opacity duration-500"
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

      {/* 4. Okunabilirlik perdesi — metin ve paneller arka plan görselinin üstünde kalsın */}
      {(settings.type === "custom_video" || settings.type === "custom_image") && (
        <div className="absolute inset-0 bg-black/80 pointer-events-none z-10" />
      )}

      {/* 5. Canvas Parçacıkları */}
      {settings.particlesEnabled && (
        <canvas ref={canvasRef} className="absolute inset-0 w-full h-full z-10" />
      )}

      {/* 6. Vignette */}
      <div className="absolute inset-0 bg-gradient-to-t from-black/45 via-transparent to-black/25 pointer-events-none z-10" />
    </div>
  );
}