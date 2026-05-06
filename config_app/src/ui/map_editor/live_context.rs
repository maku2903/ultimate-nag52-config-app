use std::{
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use backend::diag::Nag52Diag;
use crate::ui::diagnostic_format::{bool_u8_label, gear_label, profile_label, tcc_state_label};
use eframe::egui::{self, Color32, Grid, Ui};
use packed_struct::{prelude::PackedStruct, PackedStructSlice};

const RLI_MAP_LIVE_CONTEXT: u8 = 0x26;
const PAYLOAD_SIZE: usize = 30;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const BACKOFF_INTERVAL: Duration = Duration::from_millis(1000);
const REQUEST_TIMEOUT: Duration = Duration::from_millis(1000);
const BACKOFF_AFTER_ERRORS: u8 = 3;
const DISABLE_AFTER_ERRORS: u8 = 5;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, PackedStruct)]
#[packed_struct(endian = "lsb")]
pub struct MapLiveContext {
    pub valid_flags: u32,
    pub actual_gear: u8,
    pub target_gear: u8,
    pub profile: u8,
    pub pedal_pos_raw: u8,
    pub pedal_pos_percent: u8,
    pub input_rpm: u16,
    pub engine_rpm: u16,
    pub output_rpm: u16,
    pub atf_temp_c: i16,
    pub tcc_target_pressure_mbar: u16,
    pub tcc_current_pressure_mbar: u16,
    pub tcc_requested_pressure_mbar: u16,
    pub tcc_load_percent: i16,
    pub tcc_target_state: u8,
    pub tcc_current_state: u8,
    pub active_shift_circuits: u8,
    pub shift_active: u8,
    pub shift_phase: u8,
}

impl MapLiveContext {
    fn read(nag: &Nag52Diag) -> LiveContextPollResult {
        match nag.try_with_kwp(|server| {
            server.kwp_read_custom_local_identifier(RLI_MAP_LIVE_CONTEXT)
        }) {
            Ok(Some(response)) => {
                if response.len() != PAYLOAD_SIZE {
                    LiveContextPollResult::Error(format!(
                        "Invalid live data payload size: got {}, expected {}",
                        response.len(),
                        PAYLOAD_SIZE
                    ))
                } else {
                    Self::unpack_from_slice(&response)
                        .map(LiveContextPollResult::Data)
                        .unwrap_or_else(|_| {
                            LiveContextPollResult::Error("Failed to parse live data payload".into())
                        })
                }
            }
            Ok(None) => LiveContextPollResult::Busy,
            Err(err) => LiveContextPollResult::Error(err.to_string()),
        }
    }

    fn flag_is_set(&self, bit: u8) -> bool {
        self.valid_flags & (1u32 << bit) != 0
    }
}

enum LiveContextPollResult {
    Data(MapLiveContext),
    Busy,
    Error(String),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum PollState {
    Idle,
    InFlight,
    Backoff,
    Disabled,
}

pub struct MapLiveContextState {
    open: bool,
    context: Option<MapLiveContext>,
    error: Option<String>,
    stale: bool,
    state: PollState,
    worker: Option<Receiver<LiveContextPollResult>>,
    request_started: Option<Instant>,
    request_timed_out: bool,
    next_poll: Option<Instant>,
    consecutive_errors: u8,
}

impl Default for MapLiveContextState {
    fn default() -> Self {
        Self {
            open: false,
            context: None,
            error: None,
            stale: false,
            state: PollState::Idle,
            worker: None,
            request_started: None,
            request_timed_out: false,
            next_poll: None,
            consecutive_errors: 0,
        }
    }
}

impl MapLiveContextState {
    pub fn open(&mut self, nag: &Nag52Diag) {
        self.open = true;
        if self.state == PollState::Disabled {
            self.state = PollState::Idle;
            self.consecutive_errors = 0;
        }
        self.start_request(nag, true);
    }

    pub fn show_modal(&mut self, ctx: &egui::Context, nag: &Nag52Diag, skip_poll: bool) {
        if !self.open {
            return;
        }

        self.collect_worker_result();
        self.update_timeout();
        if !skip_poll {
            self.maybe_start_scheduled_request(nag);
        }
        ctx.request_repaint_after(Duration::from_millis(50));

        egui::Modal::new(egui::Id::new("map-live-context-modal")).show(ctx, |ui| {
            ui.set_min_width(560.0);
            ui.heading("Map live data");
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Refresh now").clicked() {
                    self.start_request(nag, true);
                }
                if ui.button("Close").clicked() {
                    self.close();
                }
            });

            self.show_status(ui);

            if let Some(error) = &self.error {
                ui.colored_label(Color32::YELLOW, error);
            }

            if let Some(context) = self.context {
                ui.separator();
                Grid::new("map-live-context-values")
                    .striped(true)
                    .num_columns(2)
                    .show(ui, |ui| {
                        live_context_row(
                            ui,
                            "Valid flags",
                            format!("0x{:08X}", context.valid_flags),
                        );
                        live_context_row(ui, "Actual gear", gear_label(context.actual_gear));
                        live_context_row(ui, "Target gear", gear_label(context.target_gear));
                        live_context_row(ui, "Profile", profile_label(context.profile));
                        live_context_row(ui, "Pedal raw", format!("{}", context.pedal_pos_raw));
                        live_context_row(
                            ui,
                            "Pedal position",
                            format!("{} %", context.pedal_pos_percent),
                        );
                        live_context_row(ui, "Input RPM", format!("{} RPM", context.input_rpm));
                        live_context_row(ui, "Engine RPM", format!("{} RPM", context.engine_rpm));
                        live_context_row(ui, "Output RPM", format!("{} RPM", context.output_rpm));
                        live_context_row(
                            ui,
                            "ATF temperature",
                            format!("{} C", context.atf_temp_c),
                        );
                        live_context_row(
                            ui,
                            "TCC target pressure",
                            format!("{} mBar", context.tcc_target_pressure_mbar),
                        );
                        live_context_row(
                            ui,
                            "TCC current pressure",
                            format!("{} mBar", context.tcc_current_pressure_mbar),
                        );
                        live_context_row(
                            ui,
                            "TCC requested pressure",
                            format!("{} mBar", context.tcc_requested_pressure_mbar),
                        );
                        live_context_row(ui, "TCC load", format!("{} %", context.tcc_load_percent));
                        live_context_row(
                            ui,
                            "TCC target state",
                            tcc_state_label(context.tcc_target_state),
                        );
                        live_context_row(
                            ui,
                            "TCC current state",
                            tcc_state_label(context.tcc_current_state),
                        );
                        live_context_row(
                            ui,
                            "Active shift circuits",
                            format!("0b{:04b}", context.active_shift_circuits),
                        );
                        live_context_row(ui, "Shift active", bool_u8_label(context.shift_active));
                        live_context_row(ui, "Shift phase", format!("{}", context.shift_phase));
                    });

                ui.separator();
                ui.strong("Validity");
                Grid::new("map-live-context-validity")
                    .striped(true)
                    .num_columns(2)
                    .show(ui, |ui| {
                        valid_flag_row(ui, "Gear", context.flag_is_set(0));
                        valid_flag_row(ui, "Profile", context.flag_is_set(1));
                        valid_flag_row(ui, "Pedal", context.flag_is_set(3));
                        valid_flag_row(ui, "Input RPM", context.flag_is_set(4));
                        valid_flag_row(ui, "Engine RPM", context.flag_is_set(5));
                        valid_flag_row(ui, "Output RPM", context.flag_is_set(6));
                        valid_flag_row(ui, "ATF temperature", context.flag_is_set(7));
                        valid_flag_row(ui, "TCC state", context.flag_is_set(8));
                        valid_flag_row(ui, "TCC requested pressure", context.flag_is_set(9));
                        valid_flag_row(ui, "TCC target pressure", context.flag_is_set(10));
                        valid_flag_row(ui, "TCC current pressure", context.flag_is_set(11));
                        valid_flag_row(ui, "TCC load", context.flag_is_set(12));
                        valid_flag_row(ui, "Shift state", context.flag_is_set(13));
                        valid_flag_row(ui, "Shift circuits", context.flag_is_set(14));
                    });
            } else {
                ui.label("No data received yet.");
            }
        });
    }

    fn close(&mut self) {
        self.open = false;
        self.worker = None;
        self.request_started = None;
        self.request_timed_out = false;
        if self.state == PollState::InFlight {
            self.state = PollState::Idle;
        }
    }

    fn maybe_start_scheduled_request(&mut self, nag: &Nag52Diag) {
        if self.state == PollState::Disabled || self.state == PollState::InFlight {
            return;
        }

        let now = Instant::now();
        if self.next_poll.map(|next| now >= next).unwrap_or(true) {
            self.start_request(nag, false);
        }
    }

    fn start_request(&mut self, nag: &Nag52Diag, force: bool) {
        self.collect_worker_result();
        self.update_timeout();

        if self.state == PollState::InFlight {
            if force {
                self.error = Some("Live data request already in flight; refresh skipped".into());
                self.stale = self.context.is_some();
            }
            return;
        }

        if self.state == PollState::Disabled && !force {
            return;
        }

        if force && self.state == PollState::Disabled {
            self.state = PollState::Idle;
            self.consecutive_errors = 0;
        }

        let nag = nag.clone();
        let (tx, rx) = mpsc::channel();
        let _ = thread::Builder::new()
            .name("map-live-context-poll".into())
            .spawn(move || {
                let _ = tx.send(MapLiveContext::read(&nag));
            });

        self.worker = Some(rx);
        self.request_started = Some(Instant::now());
        self.request_timed_out = false;
        self.state = PollState::InFlight;
    }

    fn collect_worker_result(&mut self) {
        let Some(worker) = self.worker.take() else {
            return;
        };

        match worker.try_recv() {
            Ok(result) => {
                self.request_started = None;
                self.request_timed_out = false;
                self.apply_result(result);
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.worker = Some(worker);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.request_started = None;
                self.request_timed_out = false;
                self.apply_error("Live data worker disconnected".into());
            }
        }
    }

    fn update_timeout(&mut self) {
        if self.state != PollState::InFlight || self.request_timed_out {
            return;
        }

        if self
            .request_started
            .map(|started| started.elapsed() >= REQUEST_TIMEOUT)
            .unwrap_or(false)
        {
            self.request_timed_out = true;
            self.stale = self.context.is_some();
            self.error = Some(
                "Live data request timed out; waiting for diagnostic call to finish".into(),
            );
        }
    }

    fn apply_result(&mut self, result: LiveContextPollResult) {
        match result {
            LiveContextPollResult::Data(context) => {
                self.context = Some(context);
                self.error = None;
                self.stale = false;
                self.consecutive_errors = 0;
                self.state = PollState::Idle;
                self.next_poll = Some(Instant::now() + POLL_INTERVAL);
            }
            LiveContextPollResult::Busy => {
                self.error = Some("Diagnostics busy; live data poll skipped".into());
                self.stale = self.context.is_some();
                self.state = PollState::Idle;
                self.next_poll = Some(Instant::now() + POLL_INTERVAL);
            }
            LiveContextPollResult::Error(error) => self.apply_error(error),
        }
    }

    fn apply_error(&mut self, error: String) {
        self.consecutive_errors = self.consecutive_errors.saturating_add(1);
        self.error = Some(error);
        self.stale = self.context.is_some();

        if self.consecutive_errors >= DISABLE_AFTER_ERRORS {
            self.state = PollState::Disabled;
            self.error = Some(format!(
                "Live data auto polling stopped after {} consecutive errors. Use Refresh now to retry.",
                self.consecutive_errors
            ));
            self.next_poll = None;
        } else {
            let interval = if self.consecutive_errors >= BACKOFF_AFTER_ERRORS {
                self.state = PollState::Backoff;
                BACKOFF_INTERVAL
            } else {
                self.state = PollState::Idle;
                POLL_INTERVAL
            };
            self.next_poll = Some(Instant::now() + interval);
        }
    }

    fn show_status(&self, ui: &mut Ui) {
        let status = match self.state {
            PollState::Idle => "Polling: idle",
            PollState::InFlight => "Polling: request in flight",
            PollState::Backoff => "Polling: backoff",
            PollState::Disabled => "Polling: disabled",
        };
        let color = match self.state {
            PollState::Idle => ui.visuals().text_color(),
            PollState::InFlight => Color32::LIGHT_BLUE,
            PollState::Backoff => Color32::YELLOW,
            PollState::Disabled => Color32::RED,
        };
        ui.colored_label(color, status);
        if self.stale {
            ui.colored_label(Color32::YELLOW, "Showing last valid snapshot");
        }
    }
}

fn live_context_row(ui: &mut Ui, key: &str, value: impl Into<egui::WidgetText>) {
    ui.strong(key);
    ui.label(value);
    ui.end_row();
}

fn valid_flag_row(ui: &mut Ui, key: &str, valid: bool) {
    ui.strong(key);
    let color = if valid {
        Color32::LIGHT_GREEN
    } else {
        ui.visuals().weak_text_color()
    };
    ui.colored_label(color, if valid { "valid" } else { "missing" });
    ui.end_row();
}
