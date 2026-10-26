use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Flex, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Cell, Clear, HighlightSpacing, List, ListItem, ListState, Paragraph, Row, Table,
    TableState, Wrap,
};

use crate::app::{AddDialog, AddStage, App, Choice, Conflict};

const TABLE_COLUMNS: [Constraint; 4] = [
    Constraint::Percentage(25),
    Constraint::Length(12),
    Constraint::Length(22),
    Constraint::Min(15),
];
const TABLE_SPACING: u16 = 1;
const SELECTION_WIDTH: u16 = 2;

#[derive(Default)]
pub struct LinkAreas {
    links: Vec<(Rect, usize)>,
}

impl LinkAreas {
    pub fn at(&self, column: u16, row: u16) -> Option<usize> {
        self.links
            .iter()
            .find(|(area, _)| area.contains(Position::new(column, row)))
            .map(|(_, index)| *index)
    }
}

// {impl}`requirements.tui.table`
// {impl}`requirements.tui.selection`
// {impl}`requirements.errors.visible`
// {impl}`requirements.errors.info`
// {impl}`requirements.help.shortcuts`
// {impl}`requirements.browser.link`
pub fn draw(frame: &mut Frame, app: &App) -> LinkAreas {
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
        let link_style = Style::default()
            .fg(if forward.is_error() {
                Color::Red
            } else {
                Color::Cyan
            })
            .add_modifier(Modifier::UNDERLINED);
        Row::new(vec![
            Cell::from(forward.spec.host.clone()),
            Cell::from(forward.spec.remote_port.to_string()),
            Cell::from(local_url(forward.spec.local_port)).style(link_style),
            Cell::from(forward.status()),
        ])
        .style(style)
    });
    let table = Table::new(rows, TABLE_COLUMNS)
        .header(
            Row::new(["SSH host", "Remote port", "Local URL", "State"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::bordered().title("Managed forwards"))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always)
        .column_spacing(TABLE_SPACING)
        .flex(Flex::Start);
    let mut state = TableState::default();
    state.select(app.selected);
    frame.render_stateful_widget(table, areas[1], &mut state);
    let links = if app.dialog.is_some() {
        LinkAreas::default()
    } else {
        link_areas(areas[1], app, state.offset())
    };

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
        .unwrap_or("a add  b browser  click URL  x remove  r refresh  i info  ? help  q quit");
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
    links
}

fn local_url(port: u16) -> String {
    format!("http://localhost:{port}")
}

// {impl}`requirements.browser.link`
fn link_areas(table_area: Rect, app: &App, offset: usize) -> LinkAreas {
    let inner = table_area.inner(Margin::new(1, 1));
    let rows_height = inner.height.saturating_sub(1);
    let [_selection_area, columns_area] =
        Layout::horizontal([Constraint::Length(SELECTION_WIDTH), Constraint::Fill(0)])
            .areas(Rect::new(inner.x, inner.y, inner.width, 1));
    let columns = Layout::horizontal(TABLE_COLUMNS)
        .spacing(TABLE_SPACING)
        .flex(Flex::Start)
        .split(columns_area);
    let url_column = columns[2];
    let mut links = Vec::new();
    for (visible_row, index) in (offset..app.forwards.len())
        .take(usize::from(rows_height))
        .enumerate()
    {
        let url_width = local_url(app.forwards[index].spec.local_port).len() as u16;
        if url_width <= url_column.width {
            links.push((
                Rect::new(url_column.x, inner.y + 1 + visible_row as u16, url_width, 1),
                index,
            ));
        }
    }
    LinkAreas { links }
}

fn help_text() -> &'static str {
    "↑/↓ or j/k  Select row\n\
     a  Add forward       b  Open selected local port in browser\n\
     Click a local URL to open that forward in the browser\n\
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
        terminal
            .draw(|frame| {
                let _ = draw(frame, app);
            })
            .unwrap();
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

    // {test}`requirements.browser.link`
    #[test]
    fn local_url_is_visible_and_only_its_text_is_clickable() {
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
        for ch in "8031".chars() {
            app.on_key(key(KeyCode::Char(ch)));
        }
        app.on_key(key(KeyCode::Enter));

        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        let mut links = LinkAreas::default();
        terminal.draw(|frame| links = draw(frame, &app)).unwrap();
        let (area, index) = links.links[0];
        assert_eq!(index, 0);
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((area.x, area.y))
                .unwrap()
                .symbol(),
            "h"
        );
        assert_eq!(links.at(area.x, area.y), Some(0));
        assert_eq!(links.at(area.right() - 1, area.y), Some(0));
        assert_eq!(links.at(area.x - 1, area.y), None);
        assert_eq!(links.at(area.x, area.y - 1), None);
        let (text, _) = rendered(&app);
        assert!(text.contains("http://localhost:8031"));

        app.on_key(key(KeyCode::Char('a')));
        terminal.draw(|frame| links = draw(frame, &app)).unwrap();
        assert_eq!(links.at(area.x, area.y), None);
    }

    // {test}`requirements.browser.link`
    #[test]
    fn scrolled_links_point_to_the_visible_forwards() {
        let mut app = App::new(
            Box::new(FailingBackend { busy: None }),
            Ok(vec!["chronos".into()]),
        );
        for port in 8000..8005 {
            app.on_key(key(KeyCode::Char('a')));
            app.on_key(key(KeyCode::Enter));
            for ch in port.to_string().chars() {
                app.on_key(key(KeyCode::Char(ch)));
            }
            app.on_key(key(KeyCode::Enter));
            app.on_key(key(KeyCode::Enter));
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        let mut links = LinkAreas::default();
        terminal.draw(|frame| links = draw(frame, &app)).unwrap();
        assert_eq!(links.links.len(), 2);
        assert_eq!(links.links[0].1, 3);
        assert_eq!(links.links[1].1, 4);
        assert_eq!(links.at(links.links[1].0.x, links.links[1].0.y), Some(4));
    }
}
