const fs = require('fs');
const path = require('path');

// SVG Logo Şablonu: 256x256 Premium 3D Zümrüt Küp + "E" Amblemi
const svgLogo = `
<svg width="256" height="256" viewBox="0 0 256 256" fill="none" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="256" y2="256" gradientUnits="userSpaceOnUse">
      <stop stop-color="#0a101d"/>
      <stop offset="1" stop-color="#05080f"/>
    </linearGradient>
    <linearGradient id="emeraldGrad" x1="32" y1="32" x2="224" y2="224" gradientUnits="userSpaceOnUse">
      <stop stop-color="#34d399"/>
      <stop offset="0.5" stop-color="#10b981"/>
      <stop offset="1" stop-color="#047857"/>
    </linearGradient>
    <linearGradient id="glow" x1="128" y1="20" x2="128" y2="236" gradientUnits="userSpaceOnUse">
      <stop stop-color="#6ee7b7" stop-opacity="0.9"/>
      <stop offset="1" stop-color="#059669" stop-opacity="0.4"/>
    </linearGradient>
    <filter id="shadow" x="10" y="10" width="236" height="236" filterUnits="userSpaceOnUse">
      <feDropShadow dx="0" dy="8" stdDeviation="16" flood-color="#10b981" flood-opacity="0.45"/>
    </filter>
  </defs>

  <!-- Arka plan yuvarlak kare -->
  <rect x="16" y="16" width="224" height="224" rx="54" fill="url(#bg)" stroke="#1e293b" stroke-width="4"/>
  <rect x="20" y="20" width="216" height="216" rx="50" fill="none" stroke="url(#emeraldGrad)" stroke-width="2" stroke-opacity="0.6"/>

  <!-- Minecraft 3D İzometrik Küp -->
  <g filter="url(#shadow)">
    <!-- Üst Yüzey -->
    <path d="M128 42L204 84L128 126L52 84L128 42Z" fill="#10b981" fill-opacity="0.85" stroke="#6ee7b7" stroke-width="4" stroke-linejoin="round"/>
    
    <!-- Sol Yüzey -->
    <path d="M52 84L128 126V210L52 168V84Z" fill="#047857" fill-opacity="0.9" stroke="#34d399" stroke-width="4" stroke-linejoin="round"/>
    
    <!-- Sağ Yüzey -->
    <path d="M128 126L204 84V168L128 210V126Z" fill="#065f46" fill-opacity="0.95" stroke="#10b981" stroke-width="4" stroke-linejoin="round"/>
    
    <!-- İç Grid / Kristal Çizgileri -->
    <path d="M128 42V126M128 126L204 84M128 126L52 84" stroke="#a7f3d0" stroke-width="3" stroke-linecap="round"/>

    <!-- Merkez Stilize 'E' Harfi -->
    <path d="M106 98H150M106 126H138M106 154H150" stroke="#ffffff" stroke-width="8" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M106 98V154" stroke="#ffffff" stroke-width="8" stroke-linecap="round"/>
  </g>
</svg>
`;

const iconsDir = path.join(__dirname, 'apps', 'desktop', 'src-tauri', 'icons');
const publicDir = path.join(__dirname, 'apps', 'desktop', 'public');

// SVG dosyasını kaydet
fs.writeFileSync(path.join(iconsDir, 'logo.svg'), svgLogo);
fs.writeFileSync(path.join(publicDir, 'logo.svg'), svgLogo);
console.log('SVG Logosu kaydedildi.');

