Template sounds: sources in gmk-music notation (crates/instance-music/README.md in the
gamekit; sound effects: docs/GUIDE.md section 9.2). The built-in synthesiser needs no
SoundFont.

Render from the gamekit root (replace _template with your game's folder):

  gmk-music render games/_template/music-src/snd_coin.txt -o games/_template/sounds/snd_coin.wav --rate 22050 --mono --tail 0.15

Check:
  gmk-music info games/_template/music-src/snd_coin.txt
  gmk-sounds games/_template/sounds/snd_*.wav
