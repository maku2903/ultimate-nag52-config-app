use std::time::{Duration, Instant};

use backend::{
    diag::Nag52Diag,
    ecu_diagnostics::{DiagError, DiagServerResult},
};
use eframe::egui::{self, Color32, Grid, Ui};
use packed_struct::{PackedStructSlice, prelude::PackedStruct};
use crate::ui::diagnostic_format::{bool_u8_label, gear_label, profile_label, tcc_state_label};

const RLI_MAP_LIVE_CONTEXT: u8 = 0x26;
const POLL_INTERVAL: Duration = Duration::from_millis(250);

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
    fn read(nag: &Nag52Diag) -> DiagServerResult<Self> {
        let response =
            nag.with_kwp(|server| server.kwp_read_custom_local_identifier(RLI_MAP_LIVE_CONTEXT))?;
        Self::unpack_from_slice(&response).map_err(|_| DiagError::InvalidResponseLength)
    }

    fn flag_is_set(&self, bit: u8) -> bool {
        self.valid_flags & (1u32 << bit) != 0
    }
}

#[derive(Default)]
pub struct MapLiveContextState {
    open: bool,
    context: Option<MapLiveContext>,
    error: Option<String>,
    last_poll: Option<Instant>,
}

impl MapLiveContextState {
    pub fn open(&mut self, nag: &Nag52Diag) {
        self.open = true;
        self.poll(nag, true);
    }

    pub fn show_modal(&mut self, ctx: &egui::Context, nag: &Nag52Diag, skip_poll: bool) {
        if !self.open {
            return;
        }

        if !skip_poll {
            self.poll(nag, false);
        }
        ctx.request_repaint_after(POLL_INTERVAL);

        egui::Modal::new(egui::Id::new("map-live-context-modal")).show(ctx, |ui| {
            ui.set_min_width(520.0);
            ui.heading("Map live data");
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Refresh now").clicked() {
                    self.poll(nag, true);
                }
                if ui.button("Close").clicked() {
                    self.open = false;
                }
            });

            if let Some(error) = &self.error {
                ui.colored_label(Color32::RED, error);
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

    fn poll(&mut self, nag: &Nag52Diag, force: bool) {
        let now = Instant::now();
        let should_poll = force
            || self
                .last_poll
                .map(|last| now.duration_since(last) >= POLL_INTERVAL)
                .unwrap_or(true);
        if !should_poll {
            return;
        }

        self.last_poll = Some(now);
        match MapLiveContext::read(nag) {
            Ok(context) => {
                self.context = Some(context);
                self.error = None;
            }
            Err(err) => {
                self.error = Some(err.to_string());
            }
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
