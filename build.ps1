<#
  EnesLauncher - tek komutla derleme + sabit exe uretimi

  Kullanim:
    .\build.ps1                 release derle, sabit exe'yi guncelle, kisayolu yenile
    .\build.ps1 -Dev            debug derle (daha hizli derler, daha yavas calisir)
    .\build.ps1 -Run            derledikten sonra launcher'i ac
    .\build.ps1 -NoShortcut     masaustu kisayoluna dokunma
    .\build.ps1 -SkipFrontend   npm run build adimini atla

  Sabit cikti (cache temizliginden etkilenmez):
    <proje ust klasoru>\EnesLauncher-App\EnesLauncher.exe

  Derleme cache'i:
    varsayilan olarak proje icindeki target/  (git'e girmez)
    .cargo\config.toml veya CARGO_TARGET_DIR ile baska yere alinabilir
#>
[CmdletBinding()]
param(
    [switch]$Dev,
    [switch]$Run,
    [switch]$NoShortcut,
    [switch]$SkipFrontend
)

$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1, yerel komutlarin (cargo/npm) ciktisini sistem kod
# sayfasiyla (tr-TR icin ibm857) cozer; ikisi de UTF-8 yazar. Turkce
# karakterlerin (Masaustu, Klasoru...) bozulmasini bu satir engeller.
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$projectRoot = $PSScriptRoot
$parent      = Split-Path $projectRoot -Parent
$appDir      = Join-Path $parent 'EnesLauncher-App'
$configName  = if ($Dev) { 'debug' } else { 'release' }
$started     = Get-Date

# cargo'nun kullanacagi target klasorunu belirle.
# Not: `cargo metadata` ciktisini ayristirmak yerine kaynagi okuyoruz; boylece
# Turkce karakterli yollarda ve farkli sistem dillerinde de dogru calisir.
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

$targetDir = Resolve-TargetDir -Root $projectRoot
$exeSource = Join-Path (Join-Path $targetDir $configName) 'enes-launcher-app.exe'
$exeTarget = Join-Path $appDir 'EnesLauncher.exe'

Write-Host ''
Write-Host "EnesLauncher build ($configName)" -ForegroundColor Cyan
Write-Host "  kaynak : $projectRoot"
Write-Host "  cache  : $targetDir"
Write-Host "  cikti  : $exeTarget"
Write-Host ''

if (-not $SkipFrontend) {
    Write-Host '==> Frontend (npm run build)' -ForegroundColor Cyan
    Push-Location (Join-Path $projectRoot 'apps\desktop')
    try {
        npm run build
        if ($LASTEXITCODE -ne 0) { throw "npm run build basarisiz (exit $LASTEXITCODE)" }
    }
    finally {
        Pop-Location
    }
}

Write-Host "==> Rust (cargo build, $configName)" -ForegroundColor Cyan
$cargoArgs = @('build', '-p', 'enes-launcher-app')
if (-not $Dev) { $cargoArgs += '--release' }

Push-Location $projectRoot
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build basarisiz (exit $LASTEXITCODE)" }
}
finally {
    Pop-Location
}

if (-not (Test-Path $exeSource)) {
    # cargo exe'yi deps/ altindan hardlink ile olusturur; bazen gecikir.
    for ($i = 0; $i -lt 20 -and -not (Test-Path $exeSource); $i++) { Start-Sleep -Milliseconds 250 }
}
if (-not (Test-Path $exeSource)) { throw "Derlenen exe bulunamadi: $exeSource" }

New-Item -ItemType Directory -Force -Path $appDir | Out-Null
Copy-Item $exeSource $exeTarget -Force

$sizeMb = [math]::Round((Get-Item $exeTarget).Length / 1MB, 1)
Write-Host ''
Write-Host "==> Hazir: $exeTarget ($sizeMb MB)" -ForegroundColor Green
Write-Host ("    sure : {0:N1} dk" -f ((Get-Date) - $started).TotalMinutes)

if (-not $NoShortcut) {
    try {
        $desktop = [Environment]::GetFolderPath('Desktop')
        $lnk     = Join-Path $desktop 'EnesLauncher.lnk'
        $shell   = New-Object -ComObject WScript.Shell
        $sc      = $shell.CreateShortcut($lnk)
        $sc.TargetPath       = $exeTarget
        $sc.WorkingDirectory = $appDir
        $sc.IconLocation     = "$exeTarget,0"
        $sc.Description      = 'EnesLauncher - Minecraft Java Edition'
        $sc.Save()
        Write-Host "    kisayol: $lnk" -ForegroundColor Green
    }
    catch {
        Write-Host "    kisayol olusturulamadi: $($_.Exception.Message)" -ForegroundColor Yellow
    }
}

if ($Run) {
    Write-Host '==> Launcher aciliyor...' -ForegroundColor Cyan
    Start-Process $exeTarget
}
