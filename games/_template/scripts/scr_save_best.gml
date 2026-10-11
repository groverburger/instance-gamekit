// scr_save_best(): keeps the best score in template.ini (next to the game)
if (global.score_now > global.best) {
    global.best = global.score_now;
    ini_open('template.ini');
    ini_write_real('scores', 'best', global.best);
    ini_close();
}
