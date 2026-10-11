// `other` is the coin; a hidden coin is waiting to reappear
if (other.visible) {
    global.score_now += 1;
    sound_play(snd_coin);
    with (other) {
        visible = false;
        alarm[0] = 40;
    }
}
