# Downloads the General MIDI SoundFonts that gmk-music renders instrument music with into
# soundfonts\ (ignored by version control):
#   default.sf2         Arachno SoundFont 1.0 by Maxime Abbey (~155 MB, from a ~137 MB zip).
#                       Free, but its author can't recommend commercial use: it contains
#                       samples by other authors, some unknown (soundfonts\README-Arachno.txt).
#   GeneralUser-GS.sf2  GeneralUser GS v2.0.3 by S. Christian Collins (~32 MB). Free to use,
#                       also commercially (soundfonts\LICENSE-GeneralUser-GS.txt); render
#                       with --soundfont soundfonts\GeneralUser-GS.sf2 for a game you sell.
# gmk-music finds soundfonts\default.sf2 in the working directory, so run it from the
# repository root (or set INSTANCE_SOUNDFONT).
#   scripts\fetch-soundfont.ps1 [-Out soundfonts]
param([string]$Out = "")
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if (-not $Out) { $Out = Join-Path $root "soundfonts" }
$rev = "97049183643d5fc5a9322a69c5b09efb667c6c3a"   # GeneralUser-GS repository, v2.0.3
$base = "https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/$rev"
$files = @(
    @{ Url = "$base/documentation/LICENSE.txt"; Dest = "LICENSE-GeneralUser-GS.txt"; Sha = "7b32efefdf95ce38a043799f0659853ddc00fbaa14d8c50f0aca16b9b8b405be" },
    @{ Url = "$base/GeneralUser-GS.sf2"; Dest = "GeneralUser-GS.sf2"; Sha = "9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe" }
)
# Arachno is only published as a zip (official page: arachnosoft.com): a Dropbox link,
# then the author's mirror
$arachnoUrls = @(
    "https://www.dropbox.com/s/2rnpya9ecb9m4jh/arachno-soundfont-10-sf2.zip?dl=1",
    "http://maxime.abbey.free.fr/mirror/arachnosoft/files/soundfonts/arachno-soundfont-10-sf2.zip"
)
$arachnoZipSha = "5c0c573abe6c5e1a3ccceba5d52a4229db24283620c9a3eb1f403018e2aa22df"
$arachnoSha = "9a57fb3b6714e69dda12390e351b087e81fc3b1eca15c6b4bbe172799f4cf3cd"

function Get-Sha($path) { (Get-FileHash -Algorithm SHA256 $path).Hash.ToLower() }

New-Item -ItemType Directory -Force $Out | Out-Null
$ProgressPreference = "SilentlyContinue"   # Invoke-WebRequest is very slow with the progress bar
foreach ($f in $files) {
    $dest = Join-Path $Out $f.Dest
    if ((Test-Path $dest) -and ((Get-Sha $dest) -eq $f.Sha)) {
        Write-Host "up to date: $dest"
        continue
    }
    $part = "$dest.part"
    Invoke-WebRequest -UseBasicParsing -Uri $f.Url -OutFile $part
    $got = Get-Sha $part
    if ($got -ne $f.Sha) {
        Remove-Item -Force $part
        throw "checksum mismatch for $($f.Url): $got"
    }
    Move-Item -Force $part $dest
    Write-Host "fetched: $dest"
}

$dest = Join-Path $Out "default.sf2"
if ((Test-Path $dest) -and ((Get-Sha $dest) -eq $arachnoSha)) {
    Write-Host "up to date: $dest"
    exit 0
}
$zip = Join-Path $Out "arachno.zip.part"
$ok = $false
foreach ($url in $arachnoUrls) {
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $zip
        if ((Get-Sha $zip) -eq $arachnoZipSha) { $ok = $true; break }
        Write-Warning "checksum mismatch: $url"
    } catch {
        Write-Warning "download failed: $url ($_)"
    }
}
if (-not $ok) {
    if (Test-Path $zip) { Remove-Item -Force $zip }
    throw "could not download the Arachno SoundFont"
}
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($zip)
try {
    foreach ($e in $archive.Entries) {
        if ($e.FullName -eq "Arachno SoundFont - Version 1.0.sf2") {
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile($e, $dest, $true)
        } elseif ($e.FullName -eq "Read Me.txt") {
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile($e, (Join-Path $Out "README-Arachno.txt"), $true)
        }
    }
} finally {
    $archive.Dispose()
}
Remove-Item -Force $zip
Write-Host "fetched: $dest"
