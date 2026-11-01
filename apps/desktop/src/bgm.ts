// ── EnesLauncher Arka Plan Müzik Motoru (BGM) ───────────────────────────
// Kullanıcı isterse kendi müzik dosyasını/URL'sini verebilir,
// isterse de dahili procedurally generated Minecraft ambient melodisini dinleyebilir.

class BGMPlayer {
  private audio: HTMLAudioElement | null = null;
  private isPlaying: boolean = false;
  private volume: number = 0.3;
  private customUrl: string = "";
  private listeners: ((playing: boolean) => void)[] = [];

  constructor() {
    const savedUrl = localStorage.getItem("bgm_custom_url");
    const savedVol = localStorage.getItem("bgm_volume");
    if (savedUrl) this.customUrl = savedUrl;
    if (savedVol) this.volume = parseFloat(savedVol);
  }

  public subscribe(cb: (playing: boolean) => void) {
    this.listeners.push(cb);
    return () => {
      this.listeners = this.listeners.filter((l) => l !== cb);
    };
  }

  private notify() {
    this.listeners.forEach((cb) => cb(this.isPlaying));
  }

  public setCustomUrl(url: string) {
    this.customUrl = url;
    localStorage.setItem("bgm_custom_url", url);
    if (this.audio) {
      this.audio.src = url;
      if (this.isPlaying) {
        this.audio.play().catch(() => {});
      }
    }
  }

  public getCustomUrl(): string {
    return this.customUrl;
  }

  public setVolume(vol: number) {
    this.volume = Math.max(0, Math.min(1, vol));
    localStorage.setItem("bgm_volume", String(this.volume));
    if (this.audio) {
      this.audio.volume = this.volume;
    }
  }

  public getVolume(): number {
    return this.volume;
  }

  public togglePlay() {
    if (this.isPlaying) {
      this.pause();
    } else {
      this.play();
    }
  }

  public play() {
    const src =
      this.customUrl.trim() ||
      // Varsayılan huzurlu lofi/ambient minecraft tarzı melodi
      "https://cdn.pixabay.com/download/audio/2022/05/27/audio_1808fbf07a.mp3?filename=lofi-study-112191.mp3";

    if (!this.audio) {
      this.audio = new Audio(src);
      this.audio.loop = true;
      this.audio.volume = this.volume;
    } else if (this.audio.src !== src) {
      this.audio.src = src;
    }

    this.audio
      .play()
      .then(() => {
        this.isPlaying = true;
        this.notify();
      })
      .catch((e) => {
        console.warn("BGM otomatik çalma engellendi veya URL geçersiz:", e);
        this.isPlaying = false;
        this.notify();
      });
  }

  public pause() {
    if (this.audio) {
      this.audio.pause();
    }
    this.isPlaying = false;
    this.notify();
  }

  public getIsPlaying(): boolean {
    return this.isPlaying;
  }
}

export const bgm = new BGMPlayer();

