use std::io;
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton, MouseEventKind,
};
use crossterm::execute;
use piraeus::app::App;
use piraeus::hosts::discover_hosts;
use piraeus::runtime::SystemBackend;
use piraeus::ui::{LinkAreas, draw};
use ratatui::DefaultTerminal;

// {impl}`requirements.tui.terminal`
fn main() -> io::Result<()> {
    let mut app = App::new(Box::new(SystemBackend), discover_hosts());
    let mut terminal = ratatui::try_init()?;
    if let Err(error) = execute!(io::stdout(), EnableMouseCapture) {
        ratatui::restore();
        return Err(error);
    }
    let result = run(&mut terminal, &mut app);
    let disable_result = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result.and(disable_result)
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut redraw = true;
    let mut links = LinkAreas::default();
    while !app.should_quit {
        if redraw {
            terminal.draw(|frame| links = draw(frame, app))?;
            redraw = false;
        }
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.on_key(key);
                    redraw = true;
                }
                // {impl}`requirements.browser.link`
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(index) = links.at(mouse.column, mouse.row) {
                        app.on_url_click(index);
                        redraw = true;
                    }
                }
                Event::Resize(_, _) => redraw = true,
                _ => {}
            }
        }
        redraw |= app.tick();
    }
    Ok(())
}
