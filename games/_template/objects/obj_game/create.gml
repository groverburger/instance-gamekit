// the controller: created once in the first room and persistent, so it lives for the
// whole game. It owns the globals, the pause, the round timer and the HUD.
global.paused = 0;
global.score_now = 0;
global.time_left = 0;
global.best = 0;
ini_open('template.ini');
global.best = ini_read_real('scores', 'best', 0);
ini_close();
