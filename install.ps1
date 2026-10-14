# kit — Online-Installer für Windows
#
#   irm https://raw.githubusercontent.com/jli-software/kit/master/install.ps1 | iex
#
# Optionen (Umgebungsvariablen):
#   $env:KIT_VERSION = 'v0.10.0'    bestimmte Version statt der neusten
#   $env:KIT_INSTALL_DIR = 'C:\Tools' Zielordner (Default: %LOCALAPPDATA%\Programs\kit)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest ist sonst sehr langsam

$repo = 'jli-software/kit'
$asset = 'kit-windows-x64.exe'
$installDir = if ($env:KIT_INSTALL_DIR) { $env:KIT_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\kit' }
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Nur 64-Bit-Windows wird unterstützt.' }

# Neuste Version über die API auflösen: die "latest/download"-Links cached
# GitHub pro Datei, kurz nach einem Release können sie gemischt alt/neu zeigen.
$version = if ($env:KIT_VERSION) { $env:KIT_VERSION }
           else { (Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest").tag_name }
$base = "https://github.com/$repo/releases/download/$version"

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("kit-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Host "kit ($version) für Windows x64 wird geladen ..."
    Invoke-WebRequest -Uri "$base/$asset" -OutFile (Join-Path $tmp 'kit.exe') -UseBasicParsing
    Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS') -UseBasicParsing

    $expected = Get-Content (Join-Path $tmp 'SHA256SUMS') |
        ForEach-Object { $parts = $_ -split '\s+'; if ($parts[1] -in @($asset, "*$asset")) { $parts[0] } } |
        Select-Object -First 1
    $actual = (Get-FileHash (Join-Path $tmp 'kit.exe') -Algorithm SHA256).Hash
    if (-not $expected -or $expected -ne $actual.ToLower()) {
        throw 'Prüfsumme stimmt nicht — Installation abgebrochen.'
    }

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    $target = Join-Path $installDir 'kit.exe'
    Move-Item (Join-Path $tmp 'kit.exe') $target -Force
    Write-Host "Installiert: $target"
}
finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

# Zielordner in den Benutzer-PATH
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$entries = if ($userPath) { $userPath -split ';' } else { @() }
if ($entries -notcontains $installDir) {
    [Environment]::SetEnvironmentVariable('Path', (($entries + $installDir) -join ';').Trim(';'), 'User')
    Write-Host "$installDir wurde zum Benutzer-PATH hinzugefügt (neues Terminal öffnen)."
}
if (($env:Path -split ';') -notcontains $installDir) { $env:Path = "$env:Path;$installDir" }

# Andere kit.exe, die vorher im PATH liegen (z. B. alte pipx-Installation)?
$others = Get-Command kit -All -CommandType Application -ErrorAction SilentlyContinue |
    Where-Object { $_.Source -ne $target }
foreach ($other in $others) {
    Write-Warning "Weitere kit-Installation gefunden: $($other.Source) — entfernen, damit nicht die alte startet."
}

$codexHome = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $HOME '.codex' }
if (-not (Test-Path (Join-Path $codexHome 'auth.json'))) {
    Write-Host ''
    Write-Host 'Für den Codex-Provider (ChatGPT-Abo) einmal mit Codex CLI anmelden:'
    Write-Host '  codex login'
}

Write-Host ''
Write-Host "Fertig — 'kit --help' zeigt alle Modi."
