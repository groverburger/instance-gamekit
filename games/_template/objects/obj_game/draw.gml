// the HUD, drawn over everything (depth -1000)
if (room != rm_game) exit;
draw_set_font(fnt_small);
draw_set_halign(fa_left);
scr_text(16, 12, 'SCORE ' + string(global.score_now), c_white);
scr_text(16, 32, 'TIME ' + string(ceil(global.time_left / room_speed)), c_white);
draw_set_halign(fa_right);
scr_text(room_width - 16, 12, 'BEST ' + string(global.best), c_yellow);
if (global.paused) {
    draw_set_alpha(0.6);
    draw_set_color(c_black);
    draw_rectangle(0, 0, room_width, room_height, false);
    draw_set_alpha(1);
    draw_set_halign(fa_center);
    draw_set_font(fnt_big);
    scr_text(room_width / 2, room_height / 2 - 20, 'PAUSED', c_white);
    draw_set_font(fnt_small);
    scr_text(room_width / 2, room_height / 2 + 24, 'ESC resume', c_silver);
}
draw_set_halign(fa_left);
