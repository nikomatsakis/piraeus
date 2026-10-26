use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap,
};

use crate::app::{AddDialog, AddStage, App, Choice, Conflict};

// {impl}`requirements.tui.table`
// {impl}`requirements.tui.selection`
// {impl}`requirements.errors.visible`
// {impl}`requirements.errors.info`
// {impl}`requirements.help.shortcuts`
pub fn draw(frame: &mut Frame, app: &App) {
    let panel_height = if app.show_help {
        12
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
        Paragraph::new("SSH port forwards").block(Block::bordered().title("Piraeus")),
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
    let footer_style = if app.message.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };
    frame.render_widget(Paragraph::new(footer).style(footer_style), areas[3]);

    if let Some(dialog) = &app.dialog {
        draw_add_dialog(frame, app, dialog);
        if app.show_help {
            let area = centered(frame.area(), 80, 14);
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
     q (main)  Quit and stop all managed forwards\n\
     Add 1/3 Host: ↑/↓ choose, type for Other, Enter next\n\
     Add 2/3 Remote: ↑/↓ recent, digits for Other, Enter next\n\
     Add 3/3 Local: ↑/↓ choice, digits for manual, Enter start\n\
     Esc goes back one stage; from Host it cancels."
}

// {impl}`requirements.forwarding.add`
fn draw_add_dialog(frame: &mut Frame, app: &App, dialog: &AddDialog) {
    match dialog.stage {
        AddStage::Host => draw_host_picker(frame, app, dialog),
        AddStage::RemotePort => draw_remote_picker(frame, app, dialog),
        AddStage::LocalPort => draw_local_picker(frame, app, dialog),
    }
}

// {impl}`requirements.forwarding.add.host`
fn draw_host_picker(frame: &mut Frame, app: &App, dialog: &AddDialog) {
    let mut items = app.hosts.clone();
    items.push(format!("Other [{}]", dialog.custom_host_input));
    draw_picker(
        frame,
        "Add forward · 1/3 · SSH host",
        "Select a saved host or type another host name.",
        items,
        dialog.host_index,
        dialog.error.as_deref().or(app.host_error.as_deref()),
        "↑/↓ choose  Type: Other  Enter: next  Esc: cancel",
    );
}

fn selected_host_label<'a>(app: &'a App, dialog: &'a AddDialog) -> &'a str {
    if dialog.host_index == app.hosts.len() {
        dialog.custom_host_input.as_str()
    } else {
        app.hosts
            .get(dialog.host_index)
            .map(String::as_str)
            .unwrap_or("")
    }
}

// {impl}`requirements.forwarding.add.remote_port`
fn draw_remote_picker(frame: &mut Frame, app: &App, dialog: &AddDialog) {
    let mut items: Vec<String> = app
        .recent_remote_ports
        .iter()
        .map(|port| port.to_string())
        .collect();
    items.push(format!("Other [{}]", dialog.remote_input));
    draw_picker(
        frame,
        "Add forward · 2/3 · Remote port",
        &format!("SSH host: {}", selected_host_label(app, dialog)),
        items,
        dialog.remote_index,
        dialog.error.as_deref(),
        "↑/↓ recent  Digits: Other  Enter: next  Esc: back",
    );
}

fn draw_picker(
    frame: &mut Frame,
    title: &str,
    context: &str,
    items: Vec<String>,
    selected: usize,
    error: Option<&str>,
    hint: &str,
) {
    let area = centered(frame.area(), 76, 17);
    frame.render_widget(Clear, area);
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(inner);
    frame.render_widget(Paragraph::new(context), parts[0]);
    let list_items: Vec<ListItem> = items.into_iter().map(ListItem::new).collect();
    let list = List::new(list_items)
        .highlight_symbol("▶ ")
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, parts[1], &mut state);
    let mut footer = Vec::new();
    if let Some(error) = error {
        footer.push(Line::from(error).style(Style::default().fg(Color::Red)));
    } else {
        footer.push(Line::from(""));
    }
    footer.push(Line::from(hint));
    frame.render_widget(Paragraph::new(footer).wrap(Wrap { trim: false }), parts[2]);
}

// {impl}`requirements.forwarding.add.local_port`
// {impl}`requirements.forwarding.default_local_port`
// {impl}`requirements.forwarding.managed_conflict_choices`
// {impl}`requirements.forwarding.unmanaged_conflict_choices`
// {impl}`requirements.forwarding.manual_local_port`
// {impl}`requirements.forwarding.port_validation`
// {impl}`requirements.forwarding.manual_conflict`
// {impl}`requirements.forwarding.suggestion_exhausted`
fn draw_local_picker(frame: &mut Frame, app: &App, dialog: &AddDialog) {
    let area = centered(frame.area(), 76, 16);
    frame.render_widget(Clear, area);
    let remote_port = dialog
        .chosen_remote_port
        .expect("local picker has remote port");
    let mut lines = vec![
        Line::from(format!("SSH host: {}", selected_host_label(app, dialog))),
        Line::from(format!("Remote port: {remote_port}")),
        Line::from(""),
    ];
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
            Choice::Manual => format!("Use alternative local port [{}]", dialog.manual_input),
        };
        let marker = if dialog.choice == *choice { 'x' } else { ' ' };
        let style = if dialog.choice == *choice {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(format!("({marker}) {text}")).style(style));
    }
    if choices.suggested.is_none() && choices.conflict != Conflict::Free {
        lines.push(
            Line::from("No higher free local port is available.")
                .style(Style::default().fg(Color::Yellow)),
        );
    }
    lines.push(Line::from(""));
    if let Some(error) = &dialog.error {
        lines.push(Line::from(error.as_str()).style(Style::default().fg(Color::Red)));
    }
    lines.push(Line::from(
        "↑/↓ choice  Digits: manual port  Enter: start  Esc: back",
    ));
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title("Add forward · 3/3 · Local port"))
            .wrap(Wrap { trim: false }),
        area,
    );
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

        fn load_recent_ports(&self) -> Result<Vec<u16>, String> {
            Ok(Vec::new())
        }

        fn save_recent_ports(&mut self, _: &[u16]) -> Result<(), String> {
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
        app.on_key(key(KeyCode::Enter));
        for ch in "8030".chars() {
            app.on_key(key(KeyCode::Char(ch)));
        }
        app.on_key(key(KeyCode::Enter));
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

    // {test}`requirements.forwarding.add.host`
    // {test}`requirements.forwarding.add.remote_port`
    // {test}`requirements.forwarding.add.local_port`
    // {test}`requirements.forwarding.unmanaged_conflict_choices`
    // {test}`requirements.forwarding.manual_local_port`
    // {test}`requirements.help.shortcuts`
    #[test]
    fn three_picker_stages_render_choices_and_help() {
        let mut app = App::new(
            Box::new(FailingBackend { busy: Some(8030) }),
            Ok(vec!["chronos".into()]),
        );
        app.on_key(key(KeyCode::Char('a')));
        let (hosts, _) = rendered(&app);
        assert!(hosts.contains("1/3 · SSH host"));
        assert!(hosts.contains("▶ chronos"));
        assert!(hosts.contains("Other ["));
        app.on_key(key(KeyCode::Down));
        let (other, _) = rendered(&app);
        assert!(other.contains("▶ Other ["));
        app.on_key(key(KeyCode::Up));
        app.on_key(key(KeyCode::Enter));
        let (remote, _) = rendered(&app);
        assert!(remote.contains("2/3 · Remote port"));
        for ch in "8030".chars() {
            app.on_key(key(KeyCode::Char(ch)));
        }
        app.on_key(key(KeyCode::Enter));
        let (dialog, _) = rendered(&app);
        assert!(dialog.contains("3/3 · Local port"));
        assert!(dialog.contains("(x) Use suggested alternative local port 8031"));
        assert!(dialog.contains("( ) Use alternative local port ["));
        assert!(!dialog.contains("replacing existing forward"));
        app.on_key(key(KeyCode::Char('?')));
        let (help, _) = rendered(&app);
        assert!(help.contains("Keyboard help"));
        assert!(help.contains("Quit and stop all managed forwards"));
    }
}
