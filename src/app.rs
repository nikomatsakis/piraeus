use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::history::{self, RECENT_PORTS_LIMIT};
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
    // {impl}`requirements.errors.visible`
    // {impl}`requirements.forwarding.startup_state`
    fn poll(&mut self) -> ProcessState;
    // {impl}`requirements.forwarding.ssh`
    fn stop(&mut self) -> Result<(), String>;
}

pub trait Backend {
    // {impl}`requirements.forwarding.detect_conflict`
    fn port_free(&self, port: u16) -> bool;
    // {impl}`requirements.forwarding.ssh`
    fn launch(&mut self, spec: &ForwardSpec) -> Result<Box<dyn ManagedProcess>, String>;
    // {impl}`requirements.browser.link`
    // {impl}`requirements.browser.open`
    fn open_browser(&mut self, port: u16) -> Result<(), String>;
    // {impl}`requirements.forwarding.add.port_history`
    fn load_recent_ports(&self) -> Result<Vec<u16>, String>;
    // {impl}`requirements.forwarding.add.port_history`
    fn save_recent_ports(&mut self, ports: &[u16]) -> Result<(), String>;
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
    // {impl}`requirements.errors.visible`
    // {impl}`requirements.tui.table`
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

    // {impl}`requirements.errors.visible`
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddStage {
    Host,
    RemotePort,
    LocalPort,
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
    pub stage: AddStage,
    pub host_index: usize,
    pub custom_host_input: String,
    pub remote_index: usize,
    pub remote_input: String,
    pub chosen_remote_port: Option<u16>,
    pub manual_input: String,
    pub choice: Choice,
    pub error: Option<String>,
}

impl Default for AddDialog {
    // {impl}`requirements.forwarding.add`
    // {impl}`requirements.forwarding.add.host`
    fn default() -> Self {
        Self {
            stage: AddStage::Host,
            host_index: 0,
            custom_host_input: String::new(),
            remote_index: 0,
            remote_input: String::new(),
            chosen_remote_port: None,
            manual_input: String::new(),
            choice: Choice::Preferred,
            error: None,
        }
    }
}

// {impl}`requirements.forwarding.add.host`
// {impl}`requirements.forwarding.add.local_port`
// {impl}`requirements.forwarding.add.remote_port`
// {impl}`requirements.tui.selection`
fn wrap_index(current: usize, count: usize, direction: isize) -> usize {
    (current as isize + direction).rem_euclid(count as isize) as usize
}

// {impl}`requirements.forwarding.add.host`
fn is_host_char(ch: char) -> bool {
    !ch.is_control() && !ch.is_whitespace() && ch != '?'
}

pub struct App {
    backend: Box<dyn Backend>,
    pub hosts: Vec<String>,
    pub host_error: Option<String>,
    pub recent_remote_ports: Vec<u16>,
    pub forwards: Vec<Forward>,
    pub selected: Option<usize>,
    pub dialog: Option<AddDialog>,
    pub show_help: bool,
    pub show_info: bool,
    pub should_quit: bool,
    pub message: Option<String>,
}

impl App {
    // {impl}`requirements.forwarding.add.remote_port`
    // {impl}`requirements.forwarding.add.port_history`
    // {impl}`requirements.forwarding.host_discovery`
    // {impl}`requirements.tui.table`
    pub fn new(backend: Box<dyn Backend>, hosts: Result<Vec<String>, String>) -> Self {
        let (hosts, host_error) = match hosts {
            Ok(hosts) => (hosts, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let (recent_remote_ports, message) = match backend.load_recent_ports() {
            Ok(ports) => (history::normalize_recent_ports(ports), None),
            Err(error) => (
                Vec::new(),
                Some(format!("Could not load recent ports: {error}")),
            ),
        };
        Self {
            backend,
            hosts,
            host_error,
            recent_remote_ports,
            forwards: Vec::new(),
            selected: None,
            dialog: None,
            show_help: false,
            show_info: false,
            should_quit: false,
            message,
        }
    }

    // {impl}`requirements.forwarding.add.remote_port`
    pub fn displayed_recent_ports(&self) -> Vec<u16> {
        let mut ports = self.recent_remote_ports.clone();
        ports.sort_unstable();
        ports
    }

    // {impl}`requirements.tui.selection`
    pub fn selected_forward(&self) -> Option<&Forward> {
        self.selected.and_then(|index| self.forwards.get(index))
    }

    // {impl}`requirements.forwarding.detect_conflict`
    // {impl}`requirements.forwarding.default_local_port`
    // {impl}`requirements.forwarding.managed_conflict_choices`
    // {impl}`requirements.forwarding.manual_local_port`
    // {impl}`requirements.forwarding.suggestion_exhausted`
    // {impl}`requirements.forwarding.unmanaged_conflict_choices`
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

    // {impl}`requirements.forwarding.detect_conflict`
    // {impl}`requirements.forwarding.manual_conflict`
    fn port_is_free(&self, port: u16) -> bool {
        !self
            .forwards
            .iter()
            .any(|forward| forward.spec.local_port == port && forward.process.is_some())
            && self.backend.port_free(port)
    }

    // {impl}`requirements.errors.visible`
    // {impl}`requirements.forwarding.startup_state`
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
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit();
            return;
        }
        // {impl}`requirements.help.shortcuts`
        if key.code == KeyCode::Char('?') {
            self.show_help = !self.show_help;
            return;
        }
        if self.show_help {
            if key.code == KeyCode::Esc {
                self.show_help = false;
            }
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
            }
            // {impl}`requirements.forwarding.quit`
            KeyCode::Char('q') => self.quit(),
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

    // {impl}`requirements.tui.selection`
    fn select_next(&mut self) {
        if self.forwards.is_empty() {
            self.selected = None;
        } else {
            self.selected =
                Some((self.selected.unwrap_or(usize::MAX).wrapping_add(1)) % self.forwards.len());
        }
        self.show_info = false;
    }

    // {impl}`requirements.tui.selection`
    fn select_previous(&mut self) {
        if self.forwards.is_empty() {
            self.selected = None;
        } else {
            let current = self.selected.unwrap_or(0);
            self.selected = Some((current + self.forwards.len() - 1) % self.forwards.len());
        }
        self.show_info = false;
    }

    // {impl}`requirements.forwarding.add`
    fn on_dialog_key(&mut self, key: KeyEvent) {
        let mut dialog = self.dialog.take().expect("dialog exists");
        if key.code == KeyCode::Esc {
            dialog.error = None;
            match dialog.stage {
                AddStage::Host => return,
                AddStage::RemotePort => dialog.stage = AddStage::Host,
                AddStage::LocalPort => dialog.stage = AddStage::RemotePort,
            }
            self.dialog = Some(dialog);
            return;
        }
        if dialog.stage == AddStage::LocalPort && key.code == KeyCode::Enter {
            self.confirm_add(dialog);
            return;
        }
        match dialog.stage {
            AddStage::Host => self.on_host_key(&mut dialog, key),
            AddStage::RemotePort => self.on_remote_port_key(&mut dialog, key),
            AddStage::LocalPort => self.on_local_port_key(&mut dialog, key),
        }
        self.dialog = Some(dialog);
    }

    // {impl}`requirements.forwarding.add.host`
    fn on_host_key(&self, dialog: &mut AddDialog, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Left => {
                dialog.host_index = wrap_index(dialog.host_index, self.hosts.len() + 1, -1);
                dialog.error = None;
            }
            KeyCode::Down | KeyCode::Right => {
                dialog.host_index = wrap_index(dialog.host_index, self.hosts.len() + 1, 1);
                dialog.error = None;
            }
            KeyCode::Char(ch) if is_host_char(ch) => {
                dialog.host_index = self.hosts.len();
                dialog.custom_host_input.push(ch);
                dialog.error = None;
            }
            KeyCode::Backspace if dialog.host_index == self.hosts.len() => {
                dialog.custom_host_input.pop();
                dialog.error = None;
            }
            KeyCode::Enter => {
                if self.selected_host(dialog).is_some() {
                    dialog.stage = AddStage::RemotePort;
                    dialog.error = None;
                } else {
                    dialog.error = Some("Enter a custom SSH host name.".into());
                }
            }
            _ => {}
        }
    }

    // {impl}`requirements.forwarding.add.host`
    fn selected_host(&self, dialog: &AddDialog) -> Option<String> {
        if dialog.host_index == self.hosts.len() {
            let host = dialog.custom_host_input.trim();
            (!host.is_empty()).then(|| host.to_owned())
        } else {
            self.hosts.get(dialog.host_index).cloned()
        }
    }

    // {impl}`requirements.forwarding.add.remote_port`
    fn on_remote_port_key(&self, dialog: &mut AddDialog, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Left => {
                dialog.remote_index =
                    wrap_index(dialog.remote_index, self.recent_remote_ports.len() + 1, -1);
                dialog.error = None;
            }
            KeyCode::Down | KeyCode::Right => {
                dialog.remote_index =
                    wrap_index(dialog.remote_index, self.recent_remote_ports.len() + 1, 1);
                dialog.error = None;
            }
            KeyCode::Char(ch) if ch.is_ascii_digit() => {
                dialog.remote_index = self.recent_remote_ports.len();
                dialog.remote_input.push(ch);
                dialog.error = None;
            }
            KeyCode::Backspace if dialog.remote_index == self.recent_remote_ports.len() => {
                dialog.remote_input.pop();
                dialog.error = None;
            }
            KeyCode::Enter => match self.selected_remote_port(dialog) {
                Ok(port) => {
                    dialog.chosen_remote_port = Some(port);
                    dialog.choice = self.port_choices(port).default;
                    dialog.stage = AddStage::LocalPort;
                    dialog.error = None;
                }
                Err(error) => dialog.error = Some(format!("Remote port: {error}")),
            },
            KeyCode::Char(_) => {
                dialog.error = Some("Remote port: enter digits from 1 through 65535.".into())
            }
            _ => {}
        }
    }

    // {impl}`requirements.forwarding.add.remote_port`
    // {impl}`requirements.forwarding.port_validation`
    fn selected_remote_port(&self, dialog: &AddDialog) -> Result<u16, &'static str> {
        if dialog.remote_index == self.recent_remote_ports.len() {
            parse_port(&dialog.remote_input)
        } else {
            self.displayed_recent_ports()
                .get(dialog.remote_index)
                .copied()
                .ok_or("Select a recent port or enter another port.")
        }
    }

    // {impl}`requirements.forwarding.add.local_port`
    // {impl}`requirements.forwarding.manual_local_port`
    // {impl}`requirements.forwarding.port_validation`
    fn on_local_port_key(&self, dialog: &mut AddDialog, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Left => self.move_local_choice(dialog, -1),
            KeyCode::Down | KeyCode::Right => self.move_local_choice(dialog, 1),
            KeyCode::Char(ch) if ch.is_ascii_digit() => {
                dialog.choice = Choice::Manual;
                dialog.manual_input.push(ch);
                dialog.error = None;
            }
            KeyCode::Backspace if dialog.choice == Choice::Manual => {
                dialog.manual_input.pop();
                dialog.error = None;
            }
            KeyCode::Char(_) => {
                dialog.error =
                    Some("Alternative local port: enter digits from 1 through 65535.".into())
            }
            _ => {}
        }
    }

    // {impl}`requirements.forwarding.add.local_port`
    fn move_local_choice(&self, dialog: &mut AddDialog, direction: isize) {
        let Some(remote_port) = dialog.chosen_remote_port else {
            return;
        };
        let options = self.port_choices(remote_port).offered;
        let current = options
            .iter()
            .position(|choice| *choice == dialog.choice)
            .unwrap_or(0);
        dialog.choice = options[wrap_index(current, options.len(), direction)];
        dialog.error = None;
    }

    // {impl}`requirements.forwarding.port_validation`
    // {impl}`requirements.forwarding.manual_conflict`
    // {impl}`requirements.forwarding.default_local_port`
    // {impl}`requirements.forwarding.manual_local_port`
    // {impl}`requirements.forwarding.managed_conflict_choices`
    // {impl}`requirements.forwarding.unmanaged_conflict_choices`
    // {impl}`requirements.forwarding.suggestion_exhausted`
    // {impl}`requirements.forwarding.add`
    fn confirm_add(&mut self, mut dialog: AddDialog) {
        let Some(host) = self.selected_host(&dialog) else {
            dialog.error = Some("Enter a custom SSH host name.".into());
            self.dialog = Some(dialog);
            return;
        };
        let Some(remote_port) = dialog.chosen_remote_port else {
            dialog.error = Some("Select a remote port.".into());
            self.dialog = Some(dialog);
            return;
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
        // {impl}`requirements.forwarding.add.remote_port`
        // {impl}`requirements.forwarding.add.port_history`
        self.recent_remote_ports.retain(|port| *port != remote_port);
        self.recent_remote_ports.insert(0, remote_port);
        self.recent_remote_ports.truncate(RECENT_PORTS_LIMIT);
        self.message = None;
        if let Err(error) = self.backend.save_recent_ports(&self.recent_remote_ports) {
            self.message = Some(format!("Could not save recent ports: {error}"));
        }
        self.dialog = None;
    }

    // {impl}`requirements.forwarding.ssh`
    // {impl}`requirements.forwarding.startup_state`
    // {impl}`requirements.errors.visible`
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

    // {impl}`requirements.forwarding.ssh`
    fn stop_forward(&mut self, index: usize) -> Result<(), String> {
        if let Some(mut process) = self.forwards[index].process.take()
            && let Err(error) = process.stop()
        {
            self.forwards[index].process = Some(process);
            return Err(error);
        }
        Ok(())
    }

    // {impl}`requirements.errors.visible`
    // {impl}`requirements.forwarding.remove`
    // {impl}`requirements.tui.selection`
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

    // {impl}`requirements.errors.visible`
    // {impl}`requirements.forwarding.refresh`
    // {impl}`requirements.forwarding.startup_state`
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

    // {impl}`requirements.browser.link`
    // {impl}`requirements.browser.open`
    // {impl}`requirements.errors.visible`
    fn open_selected(&mut self) {
        let Some(index) = self.selected else { return };
        let port = self.forwards[index].spec.local_port;
        if let Err(error) = self.backend.open_browser(port) {
            self.forwards[index].error = Some(format!("Could not open browser: {error}"));
            self.forwards[index].phase = Phase::Error;
        }
    }

    // {impl}`requirements.browser.link`
    // {impl}`requirements.errors.visible`
    // {impl}`requirements.tui.selection`
    pub fn on_url_click(&mut self, index: usize) {
        if self.dialog.is_some() || index >= self.forwards.len() {
            return;
        }
        self.selected = Some(index);
        self.show_info = false;
        self.open_selected();
    }

    // {impl}`requirements.errors.visible`
    // {impl}`requirements.forwarding.quit`
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
    // {impl}`requirements.forwarding.ssh`
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
        recent_ports: Vec<u16>,
        launch_error: Option<String>,
        browser_error: Option<String>,
    }

    struct FakeBackend(Rc<RefCell<FakeState>>);
    struct FakeProcess {
        state: Rc<RefCell<FakeState>>,
        id: usize,
    }

    impl Backend for FakeBackend {
        // {test}`requirements.forwarding.detect_conflict`
        // {test}`requirements.forwarding.manual_conflict`
        fn port_free(&self, port: u16) -> bool {
            !self.0.borrow().busy.contains(&port)
        }

        // {test}`requirements.errors.visible`
        // {test}`requirements.forwarding.ssh`
        // {test}`requirements.forwarding.startup_state`
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

        // {test}`requirements.browser.link`
        // {test}`requirements.browser.open`
        // {test}`requirements.errors.visible`
        fn open_browser(&mut self, port: u16) -> Result<(), String> {
            let mut state = self.0.borrow_mut();
            if let Some(error) = &state.browser_error {
                return Err(error.clone());
            }
            state.browser_ports.push(port);
            Ok(())
        }

        // {test}`requirements.forwarding.add.port_history`
        fn load_recent_ports(&self) -> Result<Vec<u16>, String> {
            Ok(self.0.borrow().recent_ports.clone())
        }

        // {test}`requirements.forwarding.add.port_history`
        fn save_recent_ports(&mut self, ports: &[u16]) -> Result<(), String> {
            self.0.borrow_mut().recent_ports = ports.to_vec();
            Ok(())
        }
    }

    impl ManagedProcess for FakeProcess {
        // {test}`requirements.errors.visible`
        // {test}`requirements.forwarding.startup_state`
        fn poll(&mut self) -> ProcessState {
            self.state.borrow().processes[self.id].clone()
        }

        // {test}`requirements.forwarding.quit`
        // {test}`requirements.forwarding.refresh`
        // {test}`requirements.forwarding.remove`
        fn stop(&mut self) -> Result<(), String> {
            let mut state = self.state.borrow_mut();
            state.stops.push(self.id);
            state.processes[self.id] = ProcessState::Exited("stopped".into());
            Ok(())
        }
    }

    // {test}`requirements.forwarding.add`
    fn setup() -> (App, Rc<RefCell<FakeState>>) {
        let state = Rc::new(RefCell::new(FakeState::default()));
        let app = App::new(
            Box::new(FakeBackend(state.clone())),
            Ok(vec!["chronos".into(), "mnemosyne".into()]),
        );
        (app, state)
    }

    // {test}`requirements.forwarding.add`
    // {test}`requirements.tui.selection`
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    // {test}`requirements.forwarding.add.host`
    // {test}`requirements.forwarding.add.local_port`
    // {test}`requirements.forwarding.add.remote_port`
    fn type_text(app: &mut App, text: &str) {
        for ch in text.chars() {
            app.on_key(key(KeyCode::Char(ch)));
        }
    }

    // {test}`requirements.forwarding.add`
    // {test}`requirements.forwarding.add.host`
    // {test}`requirements.forwarding.add.local_port`
    // {test}`requirements.forwarding.add.remote_port`
    fn add(app: &mut App, remote: &str, choice: Choice, manual: &str, host_index: usize) {
        app.on_key(key(KeyCode::Char('a')));
        for _ in 0..host_index {
            app.on_key(key(KeyCode::Down));
        }
        app.on_key(key(KeyCode::Enter));
        type_text(app, remote);
        app.on_key(key(KeyCode::Enter));
        if !manual.is_empty() {
            type_text(app, manual);
        } else {
            for _ in 0..4 {
                if app.dialog.as_ref().unwrap().choice == choice {
                    break;
                }
                app.on_key(key(KeyCode::Down));
            }
            assert_eq!(app.dialog.as_ref().unwrap().choice, choice);
        }
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
    // {test}`requirements.forwarding.add.host`
    // {test}`requirements.forwarding.add.remote_port`
    // {test}`requirements.forwarding.add.local_port`
    #[test]
    fn keyboard_can_choose_host_enter_port_and_select_radio_option() {
        let (mut app, _) = setup();
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.dialog.as_ref().unwrap().stage, AddStage::Host);
        app.on_key(key(KeyCode::Down));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.dialog.as_ref().unwrap().stage, AddStage::RemotePort);
        type_text(&mut app, "8030");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.dialog.as_ref().unwrap().stage, AddStage::LocalPort);
        type_text(&mut app, "8031");
        assert_eq!(app.dialog.as_ref().unwrap().choice, Choice::Manual);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.forwards[0].spec.host, "mnemosyne");
        assert_eq!(app.forwards[0].spec.remote_port, 8030);
        assert_eq!(app.forwards[0].spec.local_port, 8031);
    }

    // {test}`requirements.forwarding.add.host`
    // {test}`requirements.forwarding.quit`
    #[test]
    fn typing_custom_host_can_start_with_q() {
        let (mut app, _) = setup();
        app.on_key(key(KeyCode::Char('a')));
        type_text(&mut app, "qa.example");
        assert!(!app.should_quit);
        assert_eq!(app.dialog.as_ref().unwrap().host_index, app.hosts.len());
        app.on_key(key(KeyCode::Enter));
        type_text(&mut app, "8030");
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.forwards[0].spec.host, "qa.example");
    }

    // {test}`requirements.forwarding.add.host`
    #[test]
    fn custom_host_works_without_saved_hosts() {
        let state = Rc::new(RefCell::new(FakeState::default()));
        let mut app = App::new(
            Box::new(FakeBackend(state)),
            Err("SSH config unavailable".into()),
        );
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.dialog.as_ref().unwrap().host_index, 0);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.dialog.as_ref().unwrap().stage, AddStage::Host);
        assert!(app.dialog.as_ref().unwrap().error.is_some());
        type_text(&mut app, "remote.example");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.dialog.as_ref().unwrap().stage, AddStage::RemotePort);
    }

    // {test}`requirements.forwarding.add.remote_port`
    // {test}`requirements.forwarding.add.port_history`
    #[test]
    fn recent_remote_ports_survive_removal_and_new_app_instance() {
        let (mut app, state) = setup();
        add(&mut app, "8030", Choice::Preferred, "", 0);
        add(&mut app, "9000", Choice::Preferred, "", 0);
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.recent_remote_ports, [9000, 8030]);
        assert_eq!(app.displayed_recent_ports(), [8030, 9000]);
        drop(app);

        let mut app = App::new(Box::new(FakeBackend(state)), Ok(vec!["chronos".into()]));
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.dialog.as_ref().unwrap().stage, AddStage::RemotePort);
        assert_eq!(app.recent_remote_ports, [9000, 8030]);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.dialog.as_ref().unwrap().chosen_remote_port, Some(8030));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.recent_remote_ports, [8030, 9000]);
    }

    // {test}`requirements.forwarding.port_validation`
    #[test]
    fn invalid_remote_and_manual_ports_keep_dialog_open() {
        let (mut app, state) = setup();
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('x')));
        assert!(
            app.dialog
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("enter digits")
        );
        app.dialog = None;
        add(&mut app, "65536", Choice::Preferred, "", 0);
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
    fn control_c_stops_forwards_even_while_dialog_is_open() {
        let (mut app, state) = setup();
        add(&mut app, "8000", Choice::Preferred, "", 0);
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
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

    // {test}`requirements.browser.link`
    #[test]
    // {test}`requirements.tui.selection`
    fn clicking_url_opens_the_clicked_forward_and_selects_its_row() {
        let (mut app, state) = setup();
        add(&mut app, "8030", Choice::Manual, "8031", 0);
        add(&mut app, "8040", Choice::Manual, "8041", 1);
        app.on_url_click(0);
        assert_eq!(state.borrow().browser_ports, [8031]);
        assert_eq!(app.selected, Some(0));
        app.on_url_click(1);
        assert_eq!(state.borrow().browser_ports, [8031, 8041]);
        assert_eq!(app.selected, Some(1));
        app.on_key(key(KeyCode::Char('a')));
        app.on_url_click(0);
        assert_eq!(state.borrow().browser_ports, [8031, 8041]);
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

    // {test}`requirements.forwarding.add.port_history`
    // {test}`requirements.forwarding.add.remote_port`
    #[test]
    fn recent_ports_keep_22_most_recent_and_numeric_selection() {
        let mut saved: Vec<u16> = (8000..8025).rev().collect();
        saved.insert(1, 8024);
        saved.insert(2, 0);
        let state = Rc::new(RefCell::new(FakeState {
            recent_ports: saved,
            ..FakeState::default()
        }));
        let mut app = App::new(
            Box::new(FakeBackend(state.clone())),
            Ok(vec!["chronos".into()]),
        );
        let expected: Vec<u16> = (8003..8025).rev().collect();
        assert_eq!(app.recent_remote_ports, expected);
        assert_eq!(
            app.displayed_recent_ports(),
            (8003..8025).collect::<Vec<u16>>()
        );

        add(&mut app, "7000", Choice::Preferred, "", 0);
        let expected_saved: Vec<u16> = std::iter::once(7000).chain((8004..8025).rev()).collect();
        assert_eq!(state.borrow().recent_ports, expected_saved);
        drop(app);

        let mut app = App::new(Box::new(FakeBackend(state)), Ok(vec!["chronos".into()]));
        assert_eq!(app.recent_remote_ports, expected_saved);
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.dialog.as_ref().unwrap().chosen_remote_port, Some(7000));
    }
}
