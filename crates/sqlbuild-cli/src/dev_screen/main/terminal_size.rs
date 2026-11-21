//! How big the terminal is.

use std::fs::File;

/// The width and height of `terminal`, or of stderr without one; `None` when it is not a terminal or the platform cannot tell.
pub(crate) fn terminal_size(terminal: Option<&File>) -> Option<(usize, usize)> {
    #[cfg(unix)]
    {
        let size = match terminal {
            Some(terminal) => rustix::termios::tcgetwinsize(terminal),
            None => rustix::termios::tcgetwinsize(std::io::stderr()),
        };
        match size {
            Ok(size) if size.ws_col > 0 && size.ws_row > 0 => {
                Some((usize::from(size.ws_col), usize::from(size.ws_row)))
            }
            Ok(_) | Err(_) => None,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = terminal;
        None
    }
}
