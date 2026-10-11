# Downloads the General MIDI SoundFont that gmk-music renders instrument music with
# (GeneralUser GS v2.0.3 by S. Christian Collins, ~32 MB; licence: free to use, modify
# and include in software, see soundfonts\LICENSE-GeneralUser-GS.txt) into soundfonts\
# (ignored by version control). gmk-music finds soundfonts\default.sf2 in the working
# directory, so run it from the repository root (or set INSTANCE_SOUNDFONT).
#   scripts\fetch-soundfont.ps1 [-Out soundfonts]
param([string]$Out = "")
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if (-not $Out) { $Out = Join-Path $root "soundfonts" }
$rev = "97049183643d5fc5a9322a69c5b09efb667c6c3a"   # GeneralUser-GS repository, v2.0.3
$base = "https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/$rev"
$files = @(
    @{ Url = "$base/documentation/LICENSE.txt"; Dest = "LICENSE-GeneralUser-GS.txt"; Sha = "7b32efefdf95ce38a043799f0659853ddc00fbaa14d8c50f0aca16b9b8b405be" },
    @{ Url = "$base/GeneralUser-GS.sf2"; Dest = "default.sf2"; Sha = "9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe" }
)
New-Item -ItemType Directory -Force $Out | Out-Null
$ProgressPreference = "SilentlyContinue"   # Invoke-WebRequest is very slow with the progress bar
foreach ($f in $files) {
    $dest = Join-Path $Out $f.Dest
    if ((Test-Path $dest) -and ((Get-FileHash -Algorithm SHA256 $dest).Hash.ToLower() -eq $f.Sha)) {
        Write-Host "up to date: $dest"
        continue
    }
    $part = "$dest.part"
    Invoke-WebRequest -UseBasicParsing -Uri $f.Url -OutFile $part
    $got = (Get-FileHash -Algorithm SHA256 $part).Hash.ToLower()
    if ($got -ne $f.Sha) {
        Remove-Item -Force $part
        throw "checksum mismatch for $($f.Url): $got"
    }
    Move-Item -Force $part $dest
    Write-Host "fetched: $dest"
}
