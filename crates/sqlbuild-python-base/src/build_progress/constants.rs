//! Public constants of build progress.

pub const SPINNER_TICK_SECONDS: f64 = 0.1;

pub const CLEAR_LINE: &str = "\r\u{1b}[K";

pub const HIDE_CURSOR: &str = "\u{1b}[?25l";

pub const SHOW_CURSOR: &str = "\u{1b}[?25h";

pub const SPINNER_FRAMES: [&str; 10] = [
    "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}",
    "\u{2807}", "\u{280f}",
];
