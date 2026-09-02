//! Owns interactive terminal state while OmaBeam waits for one keypress.

use std::io::{self, IsTerminal, Read, Write};

use rustix::termios::{
    LocalModes, OptionalActions, QueueSelector, SpecialCodeIndex, Termios, tcflush, tcgetattr,
    tcsetattr,
};

const HIDE_CURSOR: &[u8] = b"\x1b[?25l";
const SHOW_CURSOR: &[u8] = b"\x1b[?25h";

pub(super) fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

pub(super) fn wait_for_key() -> io::Result<()> {
    let _terminal = OneKeyMode::enter()?;
    let mut input = io::stdin().lock();
    let mut key = [0];
    input.read_exact(&mut key)?;
    tcflush(&input, QueueSelector::IFlush)?;
    Ok(())
}

struct OneKeyMode {
    original: Termios,
}

impl OneKeyMode {
    fn enter() -> io::Result<Self> {
        let input = io::stdin();
        let original = tcgetattr(&input)?;
        let mut one_key = original.clone();
        one_key
            .local_modes
            .remove(LocalModes::ICANON | LocalModes::ECHO | LocalModes::ISIG);
        one_key.special_codes[SpecialCodeIndex::VMIN] = 1;
        one_key.special_codes[SpecialCodeIndex::VTIME] = 0;
        tcsetattr(&input, OptionalActions::Flush, &one_key)?;

        let mode = Self { original };
        let mut output = io::stdout().lock();
        output.write_all(HIDE_CURSOR)?;
        output.flush()?;
        Ok(mode)
    }
}

impl Drop for OneKeyMode {
    fn drop(&mut self) {
        let _ = tcsetattr(io::stdin(), OptionalActions::Now, &self.original);
        let mut output = io::stdout().lock();
        let _ = output.write_all(SHOW_CURSOR);
        let _ = output.flush();
    }
}
