use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ports::{first_free_after, parse_port};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardSpec {
    pub host: String,
    pub remote_port: u16,
    pub local_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessState {
    Starting,
    Running,
    Fault(String),
    Exited(String),
}

pub trait ManagedProcess {
    fn poll(&mut self) -> ProcessState;
    fn stop(&mut self) -> Result<(), String>;
}

pub trait Backend {
    fn port_free(&self, port: u16) -> bool;
    fn launch(&mut self, spec: &ForwardSpec) -> Result<Box<dyn ManagedProcess>, String>;
    fn open_browser(&mut self, port: u16) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Starting,
    Running,
    Error,
}

pub struct Forward {
    pub spec: ForwardSpec,
    pub phase: Phase,
    pub error: Option<String>,
    process: Option<Box<dyn ManagedProcess>>,
}

impl Forward {
    pub fn status(&self) -> String {
        if let Some(error) = &self.error {
            format!(
                "Error: {}",
                error.lines().next().unwrap_or("Unknown failure")
            )
        } else {
            match self.phase {
                Phase::Starting => "Starting".into(),
                Phase::Running => "Running".into(),
                Phase::Error => "Error".into(),
            }
        }
    }

    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Host,
    Remote,
    Choice,
    Manual,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Host => Self::Remote,
            Self::Remote => Self::Choice,
            Self::Choice => Self::Manual,
            Self::Manual => Self::Host,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Host => Self::Manual,
            Self::Remote => Self::Host,
            Self::Choice => Self::Remote,
            Self::Manual => Self::Choice,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Preferred,
    Replace,
    Suggested,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    Free,
    Managed(usize),
    Unmanaged,
}

pub struct PortChoices {
    pub conflict: Conflict,
    pub suggested: Option<u16>,
    pub offered: Vec<Choice>,
    pub default: Choice,
}

pub struct AddDialog {
    pub host_index: usize,
    pub remote_input: String,
    pub manual_input: String,
    pub choice: Choice,
    pub focus: Focus,
    pub error: Option<String>,
}

impl Default for AddDialog {
    fn default() -> Self {
        Self {
            host_index: 0,
            remote_input: String::new(),
            manual_input: String::new(),
            choice: Choice::Preferred,
            focus: Focus::Remote,
            error: None,
        }
    }
}

pub struct App {
    backend: Box<dyn Backend>,
    pub hosts: Vec<String>,
    pub host_error: Option<String>,
    pub forwards: Vec<Forward>,
    pub selected: Option<usize>,
    pub dialog: Option<AddDialog>,
    pub show_help: bool,
    pub show_info: bool,
    pub should_quit: bool,
    pub message: Option<String>,
}

impl App {
    pub fn new(backend: Box<dyn Backend>, hosts: Result<Vec<String>, String>) -> Self {
        let (hosts, host_error) = match hosts {
            Ok(hosts) => (hosts, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        Self {
            backend,
            hosts,
            host_error,
            forwards: Vec::new(),
            selected: None,
            dialog: None,
            show_help: false,
            show_info: false,
            should_quit: false,
            message: None,
        }
    }

    pub fn selected_forward(&self) -> Option<&Forward> {
        self.selected.and_then(|index| self.forwards.get(index))
    }

    // {impl}`requirements.forwarding.detect_conflict`
    pub fn port_choices(&self, preferred: u16) -> PortChoices {
        let conflict = self
            .forwards
            .iter()
            .position(|forward| forward.spec.local_port == preferred && forward.process.is_some())
            .map(Conflict::Managed)
            .unwrap_or_else(|| {
                if self.backend.port_free(preferred) {
                    Conflict::Free
                } else {
                    Conflict::Unmanaged
                }
            });
        let suggested = match conflict {
            Conflict::Free => None,
            _ => first_free_after(preferred, |port| self.port_is_free(port)),
        };
        let (offered, default) = match conflict {
            Conflict::Free => (vec![Choice::Preferred, Choice::Manual], Choice::Preferred),
            Conflict::Managed(_) => {
                let mut offered = vec![Choice::Replace];
                if suggested.is_some() {
                    offered.push(Choice::Suggested);
                }
                offered.push(Choice::Manual);
                let default = if suggested.is_some() {
                    Choice::Suggested
                } else {
                    Choice::Replace
                };
                (offered, default)
            }
            Conflict::Unmanaged => {
                let mut offered = Vec::new();
                if suggested.is_some() {
                    offered.push(Choice::Suggested);
                }
                offered.push(Choice::Manual);
                let default = if suggested.is_some() {
                    Choice::Suggested
                } else {
                    Choice::Manual
                };
                (offered, default)
            }
        };
        PortChoices {
            conflict,
            suggested,
            offered,
            default,
        }
    }

    fn port_is_free(&self, port: u16) -> bool {
        !self
            .forwards
            .iter()
            .any(|forward| forward.spec.local_port == port && forward.process.is_some())
            && self.backend.port_free(port)
    }

    // {impl}`requirements.errors.visible`
    pub fn tick(&mut self) -> bool {
        let mut changed = false;
        for forward in &mut self.forwards {
            let Some(process) = forward.process.as_mut() else {
                continue;
            };
            match process.poll() {
                ProcessState::Starting => {}
                ProcessState::Running => {
                    if forward.error.is_none() {
                        changed |= forward.phase != Phase::Running;
                        forward.phase = Phase::Running;
                    }
                }
                ProcessState::Fault(error) => {
                    changed = true;
                    forward.phase = Phase::Error;
                    forward.error = Some(error);
                }
                ProcessState::Exited(error) => {
                    changed = true;
                    forward.phase = Phase::Error;
                    forward.error = Some(error);
                    forward.process = None;
                }
            }
        }
        changed
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        // {impl}`requirements.forwarding.quit`
        if (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            || key.code == KeyCode::Char('q')
        {
            self.quit();
            return;
        }
        // {impl}`requirements.help.shortcuts`
        if key.code == KeyCode::Char('?') {
            self.show_help = !self.show_help;
            return;
        }
        if self.dialog.is_some() {
            self.on_dialog_key(key);
        } else {
            self.on_main_key(key);
        }
    }

    fn on_main_key(&mut self, key: KeyEvent) {
        match key.code {
            // {impl}`requirements.tui.selection`
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            // {impl}`requirements.forwarding.add`
            KeyCode::Char('a') => {
                self.dialog = Some(AddDialog::default());
                self.show_info = false;
                self.message = None;
            }
            // {impl}`requirements.browser.open`
            KeyCode::Char('b') => self.open_selected(),
            // {impl}`requirements.forwarding.remove`
            KeyCode::Char('x') => self.remove_selected(),
            // {impl}`requirements.forwarding.refresh`
            KeyCode::Char('r') => self.refresh(),
            // {impl}`requirements.errors.info`
            KeyCode::Char('i') => {
                self.show_info =
                    self.selected_forward().is_some_and(Forward::is_error) && !self.show_info;
            }
            _ => {}
        }
    }

    fn select_next(&mut self) {
        if self.forwards.is_empty() {
            self.selected = None;
        } else {
            self.selected =
                Some((self.selected.unwrap_or(usize::MAX).wrapping_add(1)) % self.forwards.len());
        }
        self.show_info = false;
    }

    fn select_previous(&mut self) {
        if self.forwards.is_empty() {
            self.selected = None;
        } else {
            let current = self.selected.unwrap_or(0);
            self.selected = Some((current + self.forwards.len() - 1) % self.forwards.len());
        }
        self.show_info = false;
    }

    fn on_dialog_key(&mut self, key: KeyEvent) {
        let mut dialog = self.dialog.take().expect("dialog exists");
        match key.code {
            KeyCode::Esc => return,
            KeyCode::Enter => {
                self.confirm_add(dialog);
                return;
            }
            KeyCode::Tab => dialog.focus = dialog.focus.next(),
            KeyCode::BackTab => dialog.focus = dialog.focus.previous(),
            KeyCode::Up | KeyCode::Left => self.move_dialog_selection(&mut dialog, -1),
            KeyCode::Down | KeyCode::Right => self.move_dialog_selection(&mut dialog, 1),
            KeyCode::Backspace => match dialog.focus {
                Focus::Remote => {
                    dialog.remote_input.pop();
                    self.reset_dialog_choice(&mut dialog);
                }
                Focus::Manual => {
                    dialog.manual_input.pop();
                }
                _ => {}
            },
            KeyCode::Char(ch) => match dialog.focus {
                Focus::Remote => {
                    dialog.remote_input.push(ch);
                    self.reset_dialog_choice(&mut dialog);
                }
                Focus::Manual => {
                    dialog.manual_input.push(ch);
                    dialog.choice = Choice::Manual;
                }
                Focus::Choice if ch == ' ' => self.move_dialog_selection(&mut dialog, 1),
                _ => {}
            },
            _ => {}
        }
        dialog.error = None;
        self.dialog = Some(dialog);
    }

    fn move_dialog_selection(&self, dialog: &mut AddDialog, direction: isize) {
        match dialog.focus {
            Focus::Host if !self.hosts.is_empty() => {
                let count = self.hosts.len() as isize;
                dialog.host_index =
                    (dialog.host_index as isize + direction).rem_euclid(count) as usize;
            }
            Focus::Choice => {
                if let Ok(remote_port) = parse_port(&dialog.remote_input) {
                    let options = self.port_choices(remote_port).offered;
                    let current = options
                        .iter()
                        .position(|choice| *choice == dialog.choice)
                        .unwrap_or(0) as isize;
                    let next = (current + direction).rem_euclid(options.len() as isize) as usize;
                    dialog.choice = options[next];
                }
            }
            _ => {}
        }
    }

    fn reset_dialog_choice(&self, dialog: &mut AddDialog) {
        dialog.choice = parse_port(&dialog.remote_input)
            .map(|port| self.port_choices(port).default)
            .unwrap_or(Choice::Preferred);
    }

    // {impl}`requirements.forwarding.port_validation`
    // {impl}`requirements.forwarding.manual_conflict`
    // {impl}`requirements.forwarding.default_local_port`
    // {impl}`requirements.forwarding.manual_local_port`
    // {impl}`requirements.forwarding.managed_conflict_choices`
    // {impl}`requirements.forwarding.unmanaged_conflict_choices`
    // {impl}`requirements.forwarding.suggestion_exhausted`
    fn confirm_add(&mut self, mut dialog: AddDialog) {
        let Some(host) = self.hosts.get(dialog.host_index).cloned() else {
            dialog.error = Some(
                self.host_error
                    .clone()
                    .unwrap_or_else(|| "No SSH hosts found in ~/.ssh/config.".into()),
            );
            self.dialog = Some(dialog);
            return;
        };
        let remote_port = match parse_port(&dialog.remote_input) {
            Ok(port) => port,
            Err(error) => {
                dialog.error = Some(format!("Remote port: {error}"));
                self.dialog = Some(dialog);
                return;
            }
        };
        let options = self.port_choices(remote_port);
        if !options.offered.contains(&dialog.choice) {
            dialog.choice = options.default;
            dialog.error = Some("Port availability changed; review the choices.".into());
            self.dialog = Some(dialog);
            return;
        }
        let local_port = match dialog.choice {
            Choice::Preferred if options.conflict == Conflict::Free => remote_port,
            Choice::Replace if matches!(options.conflict, Conflict::Managed(_)) => remote_port,
            Choice::Suggested => options.suggested.expect("suggested choice is offered"),
            Choice::Manual => match parse_port(&dialog.manual_input) {
                Ok(port) if self.port_is_free(port) => port,
                Ok(_) => {
                    dialog.error =
                        Some("That local port is already in use. Choose a free port.".into());
                    self.dialog = Some(dialog);
                    return;
                }
                Err(error) => {
                    dialog.error = Some(format!("Alternative local port: {error}"));
                    self.dialog = Some(dialog);
                    return;
                }
            },
            _ => {
                dialog.choice = options.default;
                dialog.error = Some("Port availability changed; review the choices.".into());
                self.dialog = Some(dialog);
                return;
            }
        };
        if let Conflict::Managed(index) = options.conflict
            && dialog.choice == Choice::Replace
        {
            if let Err(error) = self.stop_forward(index) {
                self.forwards[index].error = Some(error);
                self.forwards[index].phase = Phase::Error;
                self.dialog = None;
                return;
            }
            self.forwards.remove(index);
            self.selected = None;
        }
        if !self.port_is_free(local_port) {
            dialog.error = Some("That local port became unavailable. Review the choices.".into());
            self.dialog = Some(dialog);
            return;
        }
        self.start_forward(ForwardSpec {
            host,
            remote_port,
            local_port,
        });
        self.dialog = None;
    }

    // {impl}`requirements.forwarding.ssh`
    // {impl}`requirements.forwarding.startup_state`
    fn start_forward(&mut self, spec: ForwardSpec) {
        let (phase, error, process) = match self.backend.launch(&spec) {
            Ok(process) => (Phase::Starting, None, Some(process)),
            Err(error) => (Phase::Error, Some(error), None),
        };
        self.forwards.push(Forward {
            spec,
            phase,
            error,
            process,
        });
        self.selected = Some(self.forwards.len() - 1);
        self.show_info = false;
    }

    fn stop_forward(&mut self, index: usize) -> Result<(), String> {
        if let Some(mut process) = self.forwards[index].process.take()
            && let Err(error) = process.stop()
        {
            self.forwards[index].process = Some(process);
            return Err(error);
        }
        Ok(())
    }

    fn remove_selected(&mut self) {
        let Some(index) = self.selected else { return };
        if let Err(error) = self.stop_forward(index) {
            self.forwards[index].error = Some(error);
            self.forwards[index].phase = Phase::Error;
            return;
        }
        self.forwards.remove(index);
        self.selected = if self.forwards.is_empty() {
            None
        } else {
            Some(index.min(self.forwards.len() - 1))
        };
        self.show_info = false;
    }

    fn refresh(&mut self) {
        let mut stopped = Vec::with_capacity(self.forwards.len());
        for index in 0..self.forwards.len() {
            match self.stop_forward(index) {
                Ok(()) => stopped.push(index),
                Err(error) => {
                    self.forwards[index].error = Some(error);
                    self.forwards[index].phase = Phase::Error;
                }
            }
        }
        for index in stopped {
            let spec = self.forwards[index].spec.clone();
            match self.backend.launch(&spec) {
                Ok(process) => {
                    self.forwards[index].process = Some(process);
                    self.forwards[index].phase = Phase::Starting;
                    self.forwards[index].error = None;
                }
                Err(error) => {
                    self.forwards[index].phase = Phase::Error;
                    self.forwards[index].error = Some(error);
                }
            }
        }
    }

    fn open_selected(&mut self) {
        let Some(index) = self.selected else { return };
        let port = self.forwards[index].spec.local_port;
        if let Err(error) = self.backend.open_browser(port) {
            self.forwards[index].error = Some(format!("Could not open browser: {error}"));
            self.forwards[index].phase = Phase::Error;
        }
    }

    fn quit(&mut self) {
        let mut failed = false;
        for index in 0..self.forwards.len() {
            if let Err(error) = self.stop_forward(index) {
                self.forwards[index].error = Some(error);
                self.forwards[index].phase = Phase::Error;
                failed = true;
            }
        }
        self.should_quit = !failed;
    }
}

impl Drop for App {
    fn drop(&mut self) {
        for forward in &mut self.forwards {
            if let Some(process) = forward.process.as_mut() {
                let _ = process.stop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::rc::Rc;

    use super::*;

    #[derive(Default)]
    struct FakeState {
        busy: BTreeSet<u16>,
        launches: Vec<ForwardSpec>,
        processes: Vec<ProcessState>,
        stops: Vec<usize>,
        browser_ports: Vec<u16>,
        launch_error: Option<String>,
        browser_error: Option<String>,
    }

    struct FakeBackend(Rc<RefCell<FakeState>>);
    struct FakeProcess {
        state: Rc<RefCell<FakeState>>,
        id: usize,
    }

    impl Backend for FakeBackend {
        fn port_free(&self, port: u16) -> bool {
            !self.0.borrow().busy.contains(&port)
        }

        fn launch(&mut self, spec: &ForwardSpec) -> Result<Box<dyn ManagedProcess>, String> {
            let mut state = self.0.borrow_mut();
            if let Some(error) = &state.launch_error {
                return Err(error.clone());
            }
            let id = state.launches.len();
            state.launches.push(spec.clone());
            state.processes.push(ProcessState::Starting);
            Ok(Box::new(FakeProcess {
                state: self.0.clone(),
                id,
            }))
        }

        fn open_browser(&mut self, port: u16) -> Result<(), String> {
            let mut state = self.0.borrow_mut();
            if let Some(error) = &state.browser_error {
                return Err(error.clone());
            }
            state.browser_ports.push(port);
            Ok(())
        }
    }

    impl ManagedProcess for FakeProcess {
        fn poll(&mut self) -> ProcessState {
            self.state.borrow().processes[self.id].clone()
        }

        fn stop(&mut self) -> Result<(), String> {
            let mut state = self.state.borrow_mut();
            state.stops.push(self.id);
            state.processes[self.id] = ProcessState::Exited("stopped".into());
            Ok(())
        }
    }

    fn setup() -> (App, Rc<RefCell<FakeState>>) {
        let state = Rc::new(RefCell::new(FakeState::default()));
        let app = App::new(
            Box::new(FakeBackend(state.clone())),
            Ok(vec!["chronos".into(), "mnemosyne".into()]),
        );
        (app, state)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn add(app: &mut App, remote: &str, choice: Choice, manual: &str, host_index: usize) {
        app.on_key(key(KeyCode::Char('a')));
        let dialog = app.dialog.as_mut().unwrap();
        dialog.remote_input = remote.into();
        dialog.choice = choice;
        dialog.manual_input = manual.into();
        dialog.host_index = host_index;
        app.on_key(key(KeyCode::Enter));
    }

    // {test}`requirements.forwarding.add`
    // {test}`requirements.forwarding.default_local_port`
    // {test}`requirements.forwarding.startup_state`
    #[test]
    fn add_dialog_creates_forward_and_waits_for_readiness() {
        let (mut app, state) = setup();
        let options = app.port_choices(8030);
        assert_eq!(options.offered, [Choice::Preferred, Choice::Manual]);
        assert_eq!(options.default, Choice::Preferred);
        add(&mut app, "8030", options.default, "", 1);
        assert!(app.dialog.is_none());
        assert_eq!(
            app.forwards[0].spec,
            ForwardSpec {
                host: "mnemosyne".into(),
                remote_port: 8030,
                local_port: 8030,
            }
        );
        assert_eq!(app.forwards[0].phase, Phase::Starting);
        state.borrow_mut().processes[0] = ProcessState::Running;
        app.tick();
        assert_eq!(app.forwards[0].phase, Phase::Running);
    }

    // {test}`requirements.forwarding.add`
    #[test]
    fn keyboard_can_choose_host_enter_port_and_select_radio_option() {
        let (mut app, _) = setup();
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(key(KeyCode::BackTab));
        app.on_key(key(KeyCode::Down));
        app.on_key(key(KeyCode::Tab));
        for ch in "8030".chars() {
            app.on_key(key(KeyCode::Char(ch)));
        }
        app.on_key(key(KeyCode::Tab));
        app.on_key(key(KeyCode::Down));
        assert_eq!(app.dialog.as_ref().unwrap().choice, Choice::Manual);
        app.on_key(key(KeyCode::Tab));
        for ch in "8031".chars() {
            app.on_key(key(KeyCode::Char(ch)));
        }
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.forwards[0].spec.host, "mnemosyne");
        assert_eq!(app.forwards[0].spec.remote_port, 8030);
        assert_eq!(app.forwards[0].spec.local_port, 8031);
    }

    // {test}`requirements.forwarding.port_validation`
    #[test]
    fn invalid_remote_and_manual_ports_keep_dialog_open() {
        let (mut app, state) = setup();
        add(&mut app, "abc", Choice::Preferred, "", 0);
        assert!(
            app.dialog
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("Remote port")
        );
        assert!(state.borrow().launches.is_empty());
        app.dialog = None;
        add(&mut app, "8030", Choice::Manual, "0", 0);
        assert!(
            app.dialog
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("Alternative local port")
        );
        assert!(state.borrow().launches.is_empty());
    }

    // {test}`requirements.forwarding.manual_local_port`
    // {test}`requirements.forwarding.manual_conflict`
    #[test]
    fn manual_choice_keeps_remote_port_and_rejects_busy_local_port() {
        let (mut app, state) = setup();
        state.borrow_mut().busy.insert(8031);
        add(&mut app, "8030", Choice::Manual, "8031", 0);
        assert!(
            app.dialog
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("already in use")
        );
        assert!(state.borrow().launches.is_empty());
        app.dialog = None;
        add(&mut app, "8030", Choice::Manual, "8032", 0);
        assert_eq!(app.forwards[0].spec.remote_port, 8030);
        assert_eq!(app.forwards[0].spec.local_port, 8032);
        assert!(state.borrow().stops.is_empty());
    }

    // {test}`requirements.forwarding.manual_conflict`
    #[test]
    fn manual_choice_cannot_replace_managed_forward() {
        let (mut app, state) = setup();
        add(&mut app, "8030", Choice::Preferred, "", 0);
        add(&mut app, "9000", Choice::Manual, "8030", 1);
        assert!(
            app.dialog
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("already in use")
        );
        assert_eq!(app.forwards.len(), 1);
        assert!(state.borrow().stops.is_empty());
    }

    // {test}`requirements.forwarding.managed_conflict_choices`
    // {test}`requirements.forwarding.detect_conflict`
    #[test]
    fn managed_conflict_suggests_next_port_and_can_replace() {
        let (mut app, state) = setup();
        add(&mut app, "8030", Choice::Preferred, "", 0);
        let options = app.port_choices(8030);
        assert_eq!(options.conflict, Conflict::Managed(0));
        assert_eq!(options.suggested, Some(8031));
        assert_eq!(
            options.offered,
            [Choice::Replace, Choice::Suggested, Choice::Manual]
        );
        assert_eq!(options.default, Choice::Suggested);
        add(&mut app, "8030", Choice::Replace, "", 1);
        assert_eq!(state.borrow().stops, [0]);
        assert_eq!(app.forwards.len(), 1);
        assert_eq!(app.forwards[0].spec.host, "mnemosyne");
        assert_eq!(app.forwards[0].spec.local_port, 8030);
    }

    // {test}`requirements.forwarding.unmanaged_conflict_choices`
    // {test}`requirements.forwarding.detect_conflict`
    #[test]
    fn unmanaged_conflict_never_offers_replacement() {
        let (mut app, state) = setup();
        state.borrow_mut().busy.insert(8030);
        let options = app.port_choices(8030);
        assert_eq!(options.conflict, Conflict::Unmanaged);
        assert_eq!(options.suggested, Some(8031));
        assert_eq!(options.offered, [Choice::Suggested, Choice::Manual]);
        assert_eq!(options.default, Choice::Suggested);
        add(&mut app, "8030", options.default, "", 0);
        assert_eq!(app.forwards[0].spec.local_port, 8031);
        assert!(state.borrow().busy.contains(&8030));
        assert!(state.borrow().stops.is_empty());
    }

    // {test}`requirements.forwarding.suggestion_exhausted`
    #[test]
    fn exhausted_suggestion_leaves_manual_choice() {
        let (mut app, state) = setup();
        state.borrow_mut().busy.insert(65535);
        let options = app.port_choices(65535);
        assert_eq!(options.suggested, None);
        assert_eq!(options.offered, [Choice::Manual]);
        add(&mut app, "65535", Choice::Manual, "4000", 0);
        assert_eq!(app.forwards[0].spec.local_port, 4000);
    }

    // {test}`requirements.tui.selection`
    // {test}`requirements.forwarding.remove`
    #[test]
    fn selection_and_remove_use_the_visible_row() {
        let (mut app, state) = setup();
        add(&mut app, "8000", Choice::Preferred, "", 0);
        add(&mut app, "8001", Choice::Preferred, "", 1);
        assert_eq!(app.selected, Some(1));
        app.on_key(key(KeyCode::Up));
        assert_eq!(app.selected, Some(0));
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(state.borrow().stops, [0]);
        assert_eq!(app.forwards.len(), 1);
        assert_eq!(app.forwards[0].spec.host, "mnemosyne");
        assert_eq!(app.selected, Some(0));
    }

    // {test}`requirements.forwarding.refresh`
    // {test}`requirements.errors.visible`
    #[test]
    fn refresh_recreates_failed_and_running_rows() {
        let (mut app, state) = setup();
        add(&mut app, "8000", Choice::Preferred, "", 0);
        add(&mut app, "8001", Choice::Preferred, "", 1);
        state.borrow_mut().processes[0] = ProcessState::Exited("connection lost".into());
        app.tick();
        assert!(app.forwards[0].status().contains("connection lost"));
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(state.borrow().stops, [1]);
        assert_eq!(state.borrow().launches.len(), 4);
        assert_eq!(app.forwards.len(), 2);
        assert_eq!(app.forwards[0].phase, Phase::Starting);
        assert_eq!(app.forwards[1].phase, Phase::Starting);
    }

    // {test}`requirements.errors.visible`
    // {test}`requirements.forwarding.refresh`
    #[test]
    fn live_ssh_fault_is_visible_and_refresh_stops_its_process() {
        let (mut app, state) = setup();
        add(&mut app, "8000", Choice::Preferred, "", 0);
        state.borrow_mut().processes[0] = ProcessState::Fault("remote connection refused".into());
        app.tick();
        assert!(app.forwards[0].is_error());
        assert!(
            app.forwards[0]
                .status()
                .contains("remote connection refused")
        );
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(state.borrow().stops, [0]);
        assert_eq!(state.borrow().launches.len(), 2);
    }

    // {test}`requirements.forwarding.quit`
    #[test]
    fn quit_stops_every_managed_process() {
        let (mut app, state) = setup();
        add(&mut app, "8000", Choice::Preferred, "", 0);
        add(&mut app, "8001", Choice::Preferred, "", 1);
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
        assert_eq!(state.borrow().stops, [0, 1]);
    }

    // {test}`requirements.forwarding.quit`
    #[test]
    fn quit_stops_forwards_even_while_dialog_is_open() {
        let (mut app, state) = setup();
        add(&mut app, "8000", Choice::Preferred, "", 0);
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
        assert_eq!(state.borrow().stops, [0]);
    }

    // {test}`requirements.browser.open`
    // {test}`requirements.errors.info`
    #[test]
    fn browser_uses_actual_local_port_and_exposes_failures() {
        let (mut app, state) = setup();
        add(&mut app, "8030", Choice::Manual, "8031", 0);
        app.on_key(key(KeyCode::Char('b')));
        assert_eq!(state.borrow().browser_ports, [8031]);
        state.borrow_mut().browser_error = Some("no opener".into());
        app.on_key(key(KeyCode::Char('b')));
        assert!(app.forwards[0].is_error());
        app.on_key(key(KeyCode::Char('i')));
        assert!(app.show_info);
        assert!(
            app.forwards[0]
                .error
                .as_ref()
                .unwrap()
                .contains("no opener")
        );
    }

    // {test}`requirements.help.shortcuts`
    #[test]
    fn help_toggle_works_while_add_dialog_is_open() {
        let (mut app, _) = setup();
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.show_help);
        app.on_key(key(KeyCode::Char('?')));
        assert!(!app.show_help);
    }
}
