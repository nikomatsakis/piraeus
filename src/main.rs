use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use piraeus::app::App;
use piraeus::hosts::discover_hosts;
use piraeus::runtime::SystemBackend;
use piraeus::ui::draw;
use ratatui::DefaultTerminal;

// {impl}`requirements.tui.terminal`
fn main() -> io::Result<()> {
    let mut app = App::new(Box::new(SystemBackend), discover_hosts());
    let mut terminal = ratatui::try_init()?;
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut redraw = true;
    while !app.should_quit {
        if redraw {
            terminal.draw(|frame| draw(frame, app))?;
            redraw = false;
        }
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
            redraw = true;
        }
        redraw |= app.tick();
    }
    Ok(())
}
