if (global.paused) exit;
if (keyboard_check(vk_left) || keyboard_check(ord('A'))) x -= spd;
if (keyboard_check(vk_right) || keyboard_check(ord('D'))) x += spd;
if (keyboard_check(vk_up) || keyboard_check(ord('W'))) y -= spd;
if (keyboard_check(vk_down) || keyboard_check(ord('S'))) y += spd;
x = median(16, x, room_width - 16);
y = median(16, y, room_height - 16);
