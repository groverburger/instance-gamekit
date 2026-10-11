// Esc pauses (and resumes) a round; every object that moves checks global.paused
if (room == rm_game) {
    if (keyboard_check_pressed(vk_escape)) global.paused = !global.paused;
    if (!global.paused) {
        global.time_left -= 1;
        if (global.time_left <= 0) {
            scr_save_best();
            room_goto(rm_title);
        }
    }
}
