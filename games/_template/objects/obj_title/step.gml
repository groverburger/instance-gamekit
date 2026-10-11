// ENTER starts a round, Esc quits
if (keyboard_check_pressed(vk_enter) || keyboard_check_pressed(vk_space)) {
    global.score_now = 0;
    global.time_left = ROUND_STEPS;
    global.paused = 0;
    room_goto(rm_game);
}
if (keyboard_check_pressed(vk_escape)) game_end();
