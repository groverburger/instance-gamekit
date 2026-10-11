draw_set_halign(fa_center);
draw_set_font(fnt_big);
scr_text(room_width / 2, 180, 'TEMPLATE GAME', c_white);
draw_set_font(fnt_small);
scr_text(room_width / 2, 250, 'Collect coins for 30 seconds. Arrows move, ESC pauses.', c_silver);
if ((current_time div 500) mod 2 == 0) scr_text(room_width / 2, 320, 'PRESS ENTER', c_yellow);
scr_text(room_width / 2, 360, 'Best: ' + string(global.best), c_yellow);
draw_set_halign(fa_left);
