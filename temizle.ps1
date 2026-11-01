<#
  EnesLauncher - derleme cache temizligi
  Sabit exe'ye (EnesLauncher-App\EnesLauncher.exe) ASLA dokunmaz.

  Kullanim:
    .\temizle.ps1               14 gunden eski derleme artefaktlarini sil
    .\temizle.ps1 -Gun 7        esik degerini degistir
    .\temizle.ps1 -Incremental  sadece incremental onbellegini temizle
    .\temizle.ps1 -Hepsi        tum cache'i sil (sonraki derleme sifirdan olur)
#>
[CmdletBinding()]
param(
    [int]$Gun = 14,
    [switch]$Incremental,
    [switch]$Hepsi
)

$ErrorActionPreference = 'Stop'

try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$projectRoot = $PSScriptRoot
$parent      = Split-Path $projectRoot -Parent
$appExe      = Join-Path $parent 'EnesLauncher-App\EnesLauncher.exe'

# build.ps1 ile ayni mantik: cargo metadata ayristirmadan target klasorunu bul
function Resolve-TargetDir {
    param([string]$Root)

    $candidates = @()
    if ($env:CARGO_TARGET_DIR) { $candidates += $env:CARGO_TARGET_DIR }

    $cfgPath = Join-Path $Root '.cargo\config.toml'
    if (Test-Path $cfgPath) {
        $line = Get-Content $cfgPath -Encoding UTF8 |
                Where-Object { $_ -match '^\s*target-dir\s*=' } |
                Select-Object -First 1
        if ($line) {
            $value = ($line -split '=', 2)[1].Trim().Trim('"').Trim("'")
            if ($value) { $candidates += $value }
        }
    }

    foreach ($candidate in $candidates) {
        if (-not $candidate) { continue }
        if ([IO.Path]::IsPathRooted($candidate)) {
            return [IO.Path]::GetFullPath($candidate)
        }
        return [IO.Path]::GetFullPath((Join-Path $Root $candidate))
    }

    return [IO.Path]::GetFullPath((Join-Path $Root 'target'))
}

$cacheDir = Resolve-TargetDir -Root $projectRoot

function Get-DirSizeMb([string]$path) {
    if (-not (Test-Path $path)) { return 0 }
    $bytes = (Get-ChildItem $path -Recurse -Force -File -ErrorAction SilentlyContinue |
              Measure-Object Length -Sum).Sum
    if ($null -eq $bytes) { return 0 }
    return [math]::Round($bytes / 1MB, 1)
}

if (-not (Test-Path $cacheDir)) {
    Write-Host "Cache klasoru yok: $cacheDir" -ForegroundColor Yellow
    return
}

$before = Get-DirSizeMb $cacheDir
Write-Host ("Once : {0:N0} MB   {1}" -f $before, $cacheDir)

if ($Hepsi) {
    Write-Host '==> Tum cache siliniyor (sonraki derleme sifirdan olacak)' -ForegroundColor Cyan
    Remove-Item $cacheDir -Recurse -Force -ErrorAction SilentlyContinue
}
elseif ($Incremental) {
    Write-Host '==> incremental klasorleri siliniyor' -ForegroundColor Cyan
    Get-ChildItem $cacheDir -Directory -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -eq 'incremental' } |
        ForEach-Object { Remove-Item $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
}
else {
    $limit = (Get-Date).AddDays(-$Gun)
    Write-Host "==> $Gun gunden eski dosyalar siliniyor (esik: $limit)" -ForegroundColor Cyan

    Get-ChildItem $cacheDir -Recurse -File -Force -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTime -lt $limit } |
        Remove-Item -Force -ErrorAction SilentlyContinue

    # Bosalan klasorleri temizle (en derinden baslayarak)
    Get-ChildItem $cacheDir -Recurse -Directory -Force -ErrorAction SilentlyContinue |
        Sort-Object { $_.FullName.Length } -Descending |
        ForEach-Object {
            if (-not (Get-ChildItem $_.FullName -Force -ErrorAction SilentlyContinue)) {
                Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue
            }
        }
}

$after = Get-DirSizeMb $cacheDir
Write-Host ("Sonra: {0:N0} MB   (kazanc: {1:N0} MB)" -f $after, ($before - $after)) -ForegroundColor Green

if (Test-Path $appExe) {
    Write-Host "Sabit exe yerinde: $appExe" -ForegroundColor Green
}
