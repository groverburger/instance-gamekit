#!/usr/bin/env sh
# Downloads the General MIDI SoundFonts that gmk-music renders instrument music with into
# soundfonts/ (ignored by version control):
#   default.sf2         Arachno SoundFont 1.0 by Maxime Abbey (~155 MB, from a ~137 MB zip).
#                       Free, but its author can't recommend commercial use: it contains
#                       samples by other authors, some unknown (soundfonts/README-Arachno.txt).
#   GeneralUser-GS.sf2  GeneralUser GS v2.0.3 by S. Christian Collins (~32 MB). Free to use,
#                       also commercially (soundfonts/LICENSE-GeneralUser-GS.txt); render
#                       with --soundfont soundfonts/GeneralUser-GS.sf2 for a game you sell.
# gmk-music finds soundfonts/default.sf2 in the working directory, so run it from the
# repository root (or set INSTANCE_SOUNDFONT).
#   scripts/fetch-soundfont.sh [OUT_DIR]
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/soundfonts}
rev=97049183643d5fc5a9322a69c5b09efb667c6c3a   # GeneralUser-GS repository, v2.0.3
base=https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/$rev
sha=9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe
lic_sha=7b32efefdf95ce38a043799f0659853ddc00fbaa14d8c50f0aca16b9b8b405be
# Arachno is only published as a zip (official page: arachnosoft.com): a Dropbox link,
# then the author's mirror
arachno_urls="https://www.dropbox.com/s/2rnpya9ecb9m4jh/arachno-soundfont-10-sf2.zip?dl=1
http://maxime.abbey.free.fr/mirror/arachnosoft/files/soundfonts/arachno-soundfont-10-sf2.zip"
arachno_zip_sha=5c0c573abe6c5e1a3ccceba5d52a4229db24283620c9a3eb1f403018e2aa22df
arachno_sha=9a57fb3b6714e69dda12390e351b087e81fc3b1eca15c6b4bbe172799f4cf3cd

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}
fetch() { # url dest sha
  if [ -f "$2" ] && [ "$(sha256 "$2")" = "$3" ]; then echo "up to date: $2"; return; fi
  curl -fL --retry 3 -o "$2.part" "$1"
  got=$(sha256 "$2.part")
  if [ "$got" != "$3" ]; then rm -f "$2.part"; echo "checksum mismatch for $1: $got" >&2; exit 1; fi
  mv "$2.part" "$2"
  echo "fetched: $2"
}
mkdir -p "$out"
fetch "$base/documentation/LICENSE.txt" "$out/LICENSE-GeneralUser-GS.txt" "$lic_sha"
fetch "$base/GeneralUser-GS.sf2" "$out/GeneralUser-GS.sf2" "$sha"

if [ -f "$out/default.sf2" ] && [ "$(sha256 "$out/default.sf2")" = "$arachno_sha" ]; then
  echo "up to date: $out/default.sf2"
  exit 0
fi
zip=$out/arachno.zip.part
ok=
for url in $arachno_urls; do
  if curl -fL --retry 3 -o "$zip" "$url" && [ "$(sha256 "$zip")" = "$arachno_zip_sha" ]; then ok=1; break; fi
  echo "download failed or checksum mismatch: $url" >&2
done
if [ -z "$ok" ]; then rm -f "$zip"; exit 1; fi
tmp=$out/arachno.tmp
rm -rf "$tmp"
mkdir "$tmp"
unzip -q "$zip" "Arachno SoundFont - Version 1.0.sf2" "Read Me.txt" -d "$tmp"
mv "$tmp/Read Me.txt" "$out/README-Arachno.txt"
mv "$tmp/Arachno SoundFont - Version 1.0.sf2" "$out/default.sf2"
rm -rf "$tmp" "$zip"
echo "fetched: $out/default.sf2"
