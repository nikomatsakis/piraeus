use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph, Row, Table, TableState, Wrap};

use crate::app::{AddDialog, App, Choice, Focus};
use crate::ports::parse_port;

// {impl}`requirements.tui.table`
// {impl}`requirements.tui.selection`
// {impl}`requirements.errors.visible`
// {impl}`requirements.errors.info`
// {impl}`requirements.help.shortcuts`
pub fn draw(frame: &mut Frame, app: &App) {
    let panel_height = if app.show_help {
        10
    } else if app.show_info {
        7
    } else {
        0
    };
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(panel_height),
            Constraint::Length(2),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new("SSH port forwards").block(Block::bordered().title("pfm")),
        areas[0],
    );

    let rows = app.forwards.iter().map(|forward| {
        let style = if forward.is_error() {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
        Row::new(vec![
            forward.spec.host.clone(),
            forward.spec.remote_port.to_string(),
            forward.spec.local_port.to_string(),
            forward.status(),
        ])
        .style(style)
    });
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(27),
            Constraint::Length(14),
            Constraint::Length(13),
            Constraint::Min(15),
        ],
    )
    .header(
        Row::new(["SSH host", "Remote port", "Local port", "State"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::bordered().title("Managed forwards"))
    .row_highlight_style(Style::default().bg(Color::DarkGray))
    .highlight_symbol("▶ ");
    let mut state = TableState::default();
    state.select(app.selected);
    frame.render_stateful_widget(table, areas[1], &mut state);

    if app.show_help {
        frame.render_widget(
            Paragraph::new(help_text())
                .block(Block::bordered().title("Keyboard help"))
                .wrap(Wrap { trim: false }),
            areas[2],
        );
    } else if app.show_info {
        let detail = app
            .selected_forward()
            .and_then(|forward| forward.error.as_deref())
            .unwrap_or("No error on the selected row.");
        frame.render_widget(
            Paragraph::new(detail)
                .style(Style::default().fg(Color::Red))
                .block(Block::bordered().title("Error detail"))
                .wrap(Wrap { trim: false }),
            areas[2],
        );
    }
    let footer = app
        .message
        .as_deref()
        .unwrap_or("a add  b browser  x remove  r refresh  i info  ? help  q quit");
    frame.render_widget(Paragraph::new(footer), areas[3]);

    if let Some(dialog) = &app.dialog {
        draw_add_dialog(frame, app, dialog);
        if app.show_help {
            let area = centered(frame.area(), 76, 11);
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new(help_text())
                    .block(Block::bordered().title("Keyboard help — press ? to return"))
                    .wrap(Wrap { trim: false }),
                area,
            );
        }
    }
}

fn help_text() -> &'static str {
    "↑/↓ or j/k  Select row\n\
     a  Add forward       b  Open selected local port in browser\n\
     x  Remove selected   r  Restart all listed forwards\n\
     i  Show row error     ?  Toggle this help\n\
     q  Quit and stop all managed forwards\n\
     In Add: Tab moves focus, ↑/↓ changes host or radio choice,\n\
     Enter confirms, Esc cancels."
}

// {impl}`requirements.forwarding.add`
// {impl}`requirements.forwarding.default_local_port`
// {impl}`requirements.forwarding.managed_conflict_choices`
// {impl}`requirements.forwarding.unmanaged_conflict_choices`
// {impl}`requirements.forwarding.manual_local_port`
// {impl}`requirements.forwarding.port_validation`
// {impl}`requirements.forwarding.manual_conflict`
// {impl}`requirements.forwarding.suggestion_exhausted`
fn draw_add_dialog(frame: &mut Frame, app: &App, dialog: &AddDialog) {
    let area = centered(frame.area(), 76, 16);
    frame.render_widget(Clear, area);
    let mut lines = Vec::new();
    let host = app
        .hosts
        .get(dialog.host_index)
        .map(String::as_str)
        .unwrap_or("(none)");
    lines.push(focused_line(
        format!(
            "Host: <{host}>  ({}/{}, ↑/↓ to select)",
            if app.hosts.is_empty() {
                0
            } else {
                dialog.host_index + 1
            },
            app.hosts.len()
        ),
        dialog.focus == Focus::Host,
    ));
    lines.push(focused_line(
        format!("Remote port: [{}]", dialog.remote_input),
        dialog.focus == Focus::Remote,
    ));
    lines.push(Line::from(""));
    if let Ok(remote_port) = parse_port(&dialog.remote_input) {
        let choices = app.port_choices(remote_port);
        for choice in &choices.offered {
            let text = match choice {
                Choice::Preferred => format!("Use local port {remote_port}"),
                Choice::Replace => {
                    format!("Use local port {remote_port}, replacing existing forward")
                }
                Choice::Suggested => format!(
                    "Use suggested alternative local port {}",
                    choices.suggested.expect("suggested choice is offered")
                ),
                Choice::Manual => {
                    format!("Use alternative local port [{}]", dialog.manual_input)
                }
            };
            let marker = if dialog.choice == *choice { 'x' } else { ' ' };
            lines.push(focused_line(
                format!("({marker}) {text}"),
                (dialog.focus == Focus::Choice && dialog.choice == *choice)
                    || (dialog.focus == Focus::Manual && *choice == Choice::Manual),
            ));
        }
        if choices.suggested.is_none() && choices.conflict != crate::app::Conflict::Free {
            lines.push(
                Line::from("No higher free local port is available.")
                    .style(Style::default().fg(Color::Yellow)),
            );
        }
    } else {
        lines.push(Line::from("Enter a remote port to see local-port choices."));
        lines.push(focused_line(
            format!("Alternative local port: [{}]", dialog.manual_input),
            dialog.focus == Focus::Manual,
        ));
    }
    lines.push(Line::from(""));
    if let Some(error) = &dialog.error {
        lines.push(Line::from(error.as_str()).style(Style::default().fg(Color::Red)));
    } else if let Some(error) = &app.host_error {
        lines.push(Line::from(error.as_str()).style(Style::default().fg(Color::Red)));
    }
    lines.push(Line::from(
        "Tab: focus  ↑/↓: select  Enter: add  Esc: cancel",
    ));
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title("Add SSH port forward"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn focused_line(text: String, focused: bool) -> Line<'static> {
    let style = if focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::from(text).style(style)
}

fn centered(area: Rect, desired_width: u16, desired_height: u16) -> Rect {
    let width = desired_width.min(area.width.saturating_sub(2));
    let height = desired_height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::{Backend, ForwardSpec, ManagedProcess};

    struct FailingBackend {
        busy: Option<u16>,
    }

    impl Backend for FailingBackend {
        fn port_free(&self, port: u16) -> bool {
            self.busy != Some(port)
        }

        fn launch(&mut self, _: &ForwardSpec) -> Result<Box<dyn ManagedProcess>, String> {
            Err("SSH connection failed\nPermission denied".into())
        }

        fn open_browser(&mut self, _: u16) -> Result<(), String> {
            Ok(())
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn rendered(app: &App) -> (String, bool) {
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut lines = Vec::new();
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
            lines.push(line);
        }
        let has_red = buffer.content().iter().any(|cell| cell.fg == Color::Red);
        (lines.join("\n"), has_red)
    }

    // {test}`requirements.tui.terminal`
    // {test}`requirements.tui.table`
    // {test}`requirements.errors.visible`
    // {test}`requirements.errors.info`
    #[test]
    fn error_row_and_detail_render_in_table() {
        let mut app = App::new(
            Box::new(FailingBackend { busy: None }),
            Ok(vec!["chronos".into()]),
        );
        app.on_key(key(KeyCode::Char('a')));
        app.dialog.as_mut().unwrap().remote_input = "8030".into();
        app.on_key(key(KeyCode::Enter));
        let (table, has_red) = rendered(&app);
        assert!(table.contains("chronos"));
        assert!(table.contains("8030"));
        assert!(table.contains("Error: SSH connection failed"));
        assert!(has_red);
        app.on_key(key(KeyCode::Char('i')));
        let (detail, _) = rendered(&app);
        assert!(detail.contains("Permission denied"));
    }

    // {test}`requirements.forwarding.default_local_port`
    // {test}`requirements.forwarding.unmanaged_conflict_choices`
    // {test}`requirements.forwarding.manual_local_port`
    // {test}`requirements.help.shortcuts`
    #[test]
    fn dialog_renders_radio_choices_and_help() {
        let mut app = App::new(
            Box::new(FailingBackend { busy: Some(8030) }),
            Ok(vec!["chronos".into()]),
        );
        app.on_key(key(KeyCode::Char('a')));
        app.dialog.as_mut().unwrap().remote_input = "8030".into();
        app.dialog.as_mut().unwrap().choice = Choice::Suggested;
        let (dialog, _) = rendered(&app);
        assert!(dialog.contains("(x) Use suggested alternative local port 8031"));
        assert!(dialog.contains("( ) Use alternative local port ["));
        assert!(!dialog.contains("replacing existing forward"));
        app.on_key(key(KeyCode::Char('?')));
        let (help, _) = rendered(&app);
        assert!(help.contains("Keyboard help"));
        assert!(help.contains("Quit and stop all managed forwards"));
    }
}
