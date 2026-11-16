use eframe::egui::{self, Color32, Rounding, Margin, PopupCloseBehavior, RichText, Stroke, Vec2};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::history::{fetch_manifest_content, fetch_releases, ReleaseEntry};
use crate::i18n::{draw_flag, texts, Language, Texts};
use crate::steam::{
    inspect_steam, inspect_steam_with_override, install_libcrypto3, lock_only,
    rollback_to_manifest, uninstall_libcrypto3, unlock_and_update_steam, unlock_updates,
    LockState, ManifestChannel, SteamStatus,
};
use crate::updater::{
    check_for_update, detect_install_kind, install_app_user_local, is_app_installed_user_local,
    perform_update, uninstall_app_user_local, InstallKind, UpdateInfo, UpdateOutcome,
    CURRENT_VERSION,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Home,
    Tools,
    Options,
    HowToUse,
    Logs,
}

enum BgEvent {
    ReleasesLoaded {
        channel: ManifestChannel,
        page: u32,
        result: Result<Vec<ReleaseEntry>, String>,
    },
    Log(String),
    ActionFinished(Result<(), String>),
    AppUpdateChecked(Result<Option<UpdateInfo>, String>),
    AppUpdateFinished(Result<UpdateOutcome, String>),
}

pub struct SteamRollbackrLiteApp {
    lang: Language,
    active_tab: ActiveTab,
    status: SteamStatus,
    steam_path_override: Option<PathBuf>,
    steam_path_input: String,
    selected_channel: Option<ManifestChannel>,
    channel: ManifestChannel,
    releases: Vec<ReleaseEntry>,
    selected_idx: Option<usize>,
    details_entry: Option<ReleaseEntry>,
    available_update: Option<UpdateInfo>,
    show_update_popup: bool,
    updating_app: bool,
    current_page: u32,
    has_more: bool,
    loading_releases: bool,
    refresh_spin_start: Option<std::time::Instant>,
    busy: bool,
    filter_text: String,
    logs: Vec<String>,
    initial_ready_only: bool,
    rx: Receiver<BgEvent>,
    tx: mpsc::Sender<BgEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarIcon {
    Home,
    Tools,
    Options,
    HowToUse,
    Logs,
    Refresh,
}

fn apply_fluent_fonts(ctx: &egui::Context) {
    let candidates = [
        "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/abattis-cantarell-fonts/Cantarell-Regular.otf",
        "/usr/share/fonts/cantarell/Cantarell-Regular.otf",
        "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];
    let mut fonts = egui::FontDefinitions::default();
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("fluent_sans".to_owned(), egui::FontData::from_owned(bytes));
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                fam.insert(0, "fluent_sans".to_owned());
            }
            break;
        }
    }

    let bold_candidates = [
        "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/abattis-cantarell-fonts/Cantarell-Bold.otf",
        "/usr/share/fonts/cantarell/Cantarell-Bold.otf",
        "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ];
    let mut bold_loaded = false;
    for path in bold_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("fluent_bold".to_owned(), egui::FontData::from_owned(bytes));
            fonts.families.insert(
                egui::FontFamily::Name("fluent_bold".into()),
                vec!["fluent_bold".to_owned()],
            );
            bold_loaded = true;
            break;
        }
    }
    if !bold_loaded {
        let fallback = fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        fonts
            .families
            .insert(egui::FontFamily::Name("fluent_bold".into()), fallback);
    }

    ctx.set_fonts(fonts);
}

fn apply_fluent_dark_theme(ctx: &egui::Context) {
    apply_fluent_fonts(ctx);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(35, 35, 38);
    visuals.window_fill = Color32::from_rgb(43, 43, 46);
    visuals.extreme_bg_color = Color32::from_rgb(28, 28, 30);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(43, 43, 46);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(58, 58, 61));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(52, 52, 56);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(68, 68, 72));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(64, 64, 70);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(92, 92, 98));
    visuals.widgets.active.bg_fill = Color32::from_rgb(74, 74, 82);
    visuals.selection.bg_fill = Color32::from_rgb(48, 92, 148);
    let r = Rounding::same(6.0);
    visuals.widgets.inactive.rounding = r;
    visuals.widgets.hovered.rounding = r;
    visuals.widgets.active.rounding = r;
    visuals.widgets.noninteractive.rounding = Rounding::same(10.0);
    ctx.set_visuals(visuals);
}

fn card_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(Color32::from_rgb(43, 43, 46))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(58, 58, 61)))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(16.0))
}

fn pill_badge(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    egui::Frame::none()
        .fill(bg)
        .rounding(Rounding::same(5.0))
        .inner_margin(Margin::symmetric(8.0, 3.0))
        .show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(text).small().color(fg).strong()).extend());
        });
}

fn draw_sidebar_icon(
    painter: &egui::Painter,
    center: egui::Pos2,
    icon: SidebarIcon,
    color: Color32,
    spin_angle: f32,
) {
    let s = Stroke::new(1.4_f32, color);
    let x = center.x;
    let y = center.y;
    match icon {
        SidebarIcon::Home => {
            // Roof + house body
            painter.line_segment([egui::pos2(x - 6.0, y - 0.5), egui::pos2(x, y - 6.0)], s);
            painter.line_segment([egui::pos2(x, y - 6.0), egui::pos2(x + 6.0, y - 0.5)], s);
            painter.rect_stroke(
                egui::Rect::from_min_max(egui::pos2(x - 4.5, y - 0.5), egui::pos2(x + 4.5, y + 5.5)),
                1.0,
                s,
            );
        }
        SidebarIcon::Options => {
            // Gear icon
            painter.circle_stroke(center, 3.6, s);
            for i in 0..6 {
                let a = (i as f32) * std::f32::consts::TAU / 6.0;
                let p1 = egui::pos2(x + a.cos() * 3.8, y + a.sin() * 3.8);
                let p2 = egui::pos2(x + a.cos() * 6.0, y + a.sin() * 6.0);
                painter.line_segment([p1, p2], Stroke::new(1.7_f32, color));
            }
        }
        SidebarIcon::Tools => {
            // Crossed wrench (top-left to bottom-right) & screwdriver (bottom-left to top-right)
            // 1. Screwdriver handle (bottom-left) & shaft/tip (top-right), gapped at center
            let sd_end = egui::pos2(x - 4.8, y + 4.8);
            let sd_handle_top = egui::pos2(x - 1.7, y + 1.7);
            painter.line_segment([sd_end, sd_handle_top], Stroke::new(2.8_f32, color));
            painter.circle_filled(sd_end, 1.4_f32, color);

            let sd_shaft_bot = egui::pos2(x + 1.6, y - 1.6);
            let sd_shaft_top = egui::pos2(x + 3.8, y - 3.8);
            painter.line_segment([sd_shaft_bot, sd_shaft_top], Stroke::new(1.5_f32, color));

            // Flathead screwdriver tip at top-right
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x + 3.2, y - 4.6),
                    egui::pos2(x + 4.9, y - 5.6),
                    egui::pos2(x + 5.6, y - 4.9),
                    egui::pos2(x + 4.6, y - 3.2),
                ],
                color,
                Stroke::NONE,
            ));

            // 2. Double-ended wrench crossing from top-left to bottom-right
            painter.line_segment(
                [egui::pos2(x - 2.4, y - 2.4), egui::pos2(x + 2.4, y + 2.4)],
                Stroke::new(2.1_f32, color),
            );
            let draw_jaw = |cx: f32, cy: f32, open_angle: f32| {
                let r = 2.4_f32;
                let gap = 0.36_f32 * std::f32::consts::PI;
                let a0 = open_angle + gap;
                let a1 = open_angle + 2.0 * std::f32::consts::PI - gap;
                let steps = 12;
                let pts: Vec<egui::Pos2> = (0..=steps)
                    .map(|i| {
                        let a = a0 + (a1 - a0) * (i as f32 / steps as f32);
                        egui::pos2(cx + r * a.cos(), cy + r * a.sin())
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, Stroke::new(1.6_f32, color)));
            };
            // Top-left jaw opens toward -135 deg (-0.75 * PI)
            draw_jaw(x - 3.9, y - 3.9, -0.75_f32 * std::f32::consts::PI);
            // Bottom-right jaw opens toward +45 deg (0.25 * PI)
            draw_jaw(x + 3.9, y + 3.9, 0.25_f32 * std::f32::consts::PI);
        }
        SidebarIcon::HowToUse => {
            // Question mark in a circle
            painter.circle_stroke(center, 6.0, s);
            painter.text(
                egui::pos2(x, y + 0.5),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(10.5),
                color,
            );
        }
        SidebarIcon::Logs => {
            // Document / log lines
            painter.rect_stroke(
                egui::Rect::from_center_size(center, Vec2::new(10.0, 12.0)),
                1.5,
                s,
            );
            for dy in [-2.5, 0.0, 2.5] {
                painter.line_segment(
                    [egui::pos2(x - 2.8, y + dy), egui::pos2(x + 2.8, y + dy)],
                    Stroke::new(1.2_f32, color),
                );
            }
        }
        SidebarIcon::Refresh => {
            // Two circling arrows (like "Check for update"), rotated by spin_angle
            let r = 5.2_f32;
            let (sin_s, cos_s) = spin_angle.sin_cos();
            let rot = |px: f32, py: f32| -> egui::Pos2 {
                egui::pos2(x + px * cos_s - py * sin_s, y + px * sin_s + py * cos_s)
            };
            let arrow_stroke = Stroke::new(1.5_f32, color);

            // Draw two opposite ~135-degree arcs with arrowheads
            for base_offset in [0.0_f32, std::f32::consts::PI] {
                let start = base_offset - 2.35;
                let end = base_offset - 0.15;
                let steps = 12;
                let pts: Vec<egui::Pos2> = (0..=steps)
                    .map(|i| {
                        let a = start + (end - start) * (i as f32 / steps as f32);
                        rot(r * a.cos(), r * a.sin())
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, arrow_stroke));

                let tip = rot(r * end.cos(), r * end.sin());
                let tangent = Vec2::new(-end.sin(), end.cos());
                let radial = Vec2::new(end.cos(), end.sin());
                let wing1_local = Vec2::new(r * end.cos(), r * end.sin()) - tangent * 2.8 + radial * 2.2;
                let wing2_local = Vec2::new(r * end.cos(), r * end.sin()) - tangent * 2.8 - radial * 2.2;
                painter.line_segment([tip, rot(wing1_local.x, wing1_local.y)], arrow_stroke);
                painter.line_segment([tip, rot(wing2_local.x, wing2_local.y)], arrow_stroke);
            }
        }
    }
}

fn sidebar_item(
    ui: &mut egui::Ui,
    icon: SidebarIcon,
    label: &str,
    active: bool,
    enabled: bool,
) -> egui::Response {
    sidebar_item_spin(ui, icon, label, active, enabled, 0.0)
}

fn sidebar_item_spin(
    ui: &mut egui::Ui,
    icon: SidebarIcon,
    label: &str,
    active: bool,
    enabled: bool,
    spin_angle: f32,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 32.0);
    let (rect, resp) = ui.allocate_exact_size(
        desired,
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );

    let bg = if active {
        Color32::from_rgb(46, 46, 52)
    } else if enabled && resp.hovered() {
        Color32::from_rgb(38, 38, 42)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, Rounding::same(6.0), bg);

    if active {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + 2.0, rect.top() + 7.0),
                Vec2::new(3.0, rect.height() - 14.0),
            ),
            Rounding::same(2.0),
            Color32::from_rgb(86, 156, 244),
        );
    }

    let fg = if !enabled {
        Color32::DARK_GRAY
    } else if active {
        Color32::WHITE
    } else {
        Color32::LIGHT_GRAY
    };

    draw_sidebar_icon(
        ui.painter(),
        egui::pos2(rect.left() + 20.0, rect.center().y),
        icon,
        fg,
        spin_angle,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 36.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        fg,
    );
    resp
}

impl SteamRollbackrLiteApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_fluent_dark_theme(&cc.egui_ctx);

        let (tx, rx) = mpsc::channel();
        let status = inspect_steam();
        let steam_path_input = status
            .steam_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let default_channel = status
            .current_version
            .as_ref()
            .map(|v| v.channel)
            .unwrap_or(ManifestChannel::SteamRt64);

        let lang = Language::En;
        let t = texts(lang);

        let mut app = Self {
            lang,
            active_tab: ActiveTab::Home,
            status,
            steam_path_override: None,
            steam_path_input,
            selected_channel: None,
            channel: default_channel,
            releases: Vec::new(),
            selected_idx: None,
            details_entry: None,
            available_update: None,
            show_update_popup: false,
            updating_app: false,
            current_page: 1,
            has_more: true,
            loading_releases: false,
            refresh_spin_start: None,
            busy: false,
            filter_text: String::new(),
            logs: vec![t.ready_log.to_string()],
            initial_ready_only: true,
            rx,
            tx,
        };
        app.check_app_update();
        app
    }

    fn set_language(&mut self, new_lang: Language) {
        self.lang = new_lang;
        if self.initial_ready_only && self.logs.len() == 1 {
            self.logs[0] = texts(new_lang).ready_log.to_string();
        }
    }

    fn refresh_status(&mut self) {
        self.status = inspect_steam_with_override(self.steam_path_override.as_deref());
        if self.steam_path_override.is_none() {
            self.steam_path_input = self
                .status
                .steam_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
        }
    }

    fn check_app_update(&mut self) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = check_for_update();
            let _ = tx.send(BgEvent::AppUpdateChecked(res));
        });
    }

    fn start_app_update(&mut self) {
        let Some(info) = self.available_update.clone() else {
            return;
        };
        self.updating_app = true;
        self.initial_ready_only = false;
        self.logs.push(format!("Starting SteamRollbackrLite update to {}...", info.latest_tag));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = perform_update(&info, &mut |msg| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            });
            let _ = tx.send(BgEvent::AppUpdateFinished(res));
        });
    }

    fn load_releases(&mut self, page: u32, reset: bool) {
        if self.loading_releases {
            return;
        }
        if reset {
            self.releases.clear();
            self.selected_idx = None;
            self.current_page = 1;
            self.has_more = true;
        }
        self.loading_releases = true;
        let channel = self.channel;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = fetch_releases(channel, page);
            let _ = tx.send(BgEvent::ReleasesLoaded {
                channel,
                page,
                result,
            });
        });
    }

    fn start_rollback(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            self.initial_ready_only = false;
            self.logs.push("Error: Steam directory not found.".to_string());
            return;
        };
        let Some(idx) = self.selected_idx else {
            self.initial_ready_only = false;
            self.logs.push("Error: Please select a release first.".to_string());
            return;
        };
        let entry = self.releases[idx].clone();
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs.push(format!(
            "Fetching manifest for {} ({})...",
            entry.date_display, entry.short_sha
        ));

        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = match fetch_manifest_content(&entry.sha, entry.channel) {
                Ok((manifest, ver)) => {
                    if let Some(v) = ver {
                        log_fn(&format!("Downloaded manifest version {v}."));
                    }
                    rollback_to_manifest(&steam_dir, entry.channel, &manifest, &mut log_fn)
                }
                Err(e) => Err(e),
            };
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_unlock(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = unlock_updates(&steam_dir, &mut log_fn);
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_unlock_and_update(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            self.initial_ready_only = false;
            self.logs.push("Error: Steam directory not found.".to_string());
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs
            .push("Unlocking Steam and updating to the latest official release...".to_string());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = unlock_and_update_steam(&steam_dir, &mut log_fn);
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_lock_only(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = lock_only(&steam_dir, &mut log_fn);
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_install_libcrypto3(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs.push("Installing 32-bit libcrypto.so.3...".to_string());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = install_libcrypto3(&steam_dir, &mut |msg| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            });
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_uninstall_libcrypto3(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs.push("Removing 32-bit libcrypto.so.3...".to_string());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = uninstall_libcrypto3(&steam_dir, &mut |msg| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            });
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_install_app_user_local(&mut self) {
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs.push("Installing SteamRollbackrLite to ~/.local...".to_string());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = install_app_user_local(&mut |msg| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            });
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_uninstall_app_user_local(&mut self) {
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs.push("Removing SteamRollbackrLite from ~/.local...".to_string());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = uninstall_app_user_local(&mut |msg| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            });
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn poll_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                BgEvent::ReleasesLoaded {
                    channel,
                    page,
                    result,
                } => {
                    if channel == self.channel {
                        self.loading_releases = false;
                        match result {
                            Ok(list) => {
                                if list.len() < 40 {
                                    self.has_more = false;
                                }
                                self.current_page = page;
                                self.releases.extend(list);
                            }
                            Err(e) => {
                                self.initial_ready_only = false;
                                self.logs.push(format!("Failed to load releases: {e}"));
                            }
                        }
                    }
                }
                BgEvent::Log(msg) => {
                    self.initial_ready_only = false;
                    self.logs.push(msg);
                }
                BgEvent::ActionFinished(res) => {
                    self.busy = false;
                    if let Err(e) = res {
                        self.initial_ready_only = false;
                        self.logs.push(format!("ERROR: {e}"));
                    }
                    self.refresh_status();
                }
                BgEvent::AppUpdateChecked(res) => {
                    if let Ok(Some(info)) = res {
                        self.available_update = Some(info);
                        self.show_update_popup = true;
                    }
                }
                BgEvent::AppUpdateFinished(res) => {
                    self.updating_app = false;
                    match res {
                        Ok(UpdateOutcome::Relaunched) => {
                            std::process::exit(0);
                        }
                        Ok(UpdateOutcome::OpenedReleasePage) => {
                            self.show_update_popup = false;
                        }
                        Err(e) => {
                            self.initial_ready_only = false;
                            self.logs.push(format!("Update ERROR: {e}"));
                        }
                    }
                }
            }
        }
    }

    fn render_sidebar(&mut self, ctx: &egui::Context, t: &Texts) {
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(190.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(28, 28, 30))
                    .inner_margin(Margin::symmetric(12.0, 14.0)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("SteamRollbackrLite")
                        .size(16.0)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.add_space(14.0);

                for (tab, icon, label) in [
                    (ActiveTab::Home, SidebarIcon::Home, t.nav_home),
                    (ActiveTab::Tools, SidebarIcon::Tools, t.nav_tools),
                    (ActiveTab::Options, SidebarIcon::Options, t.nav_options),
                    (ActiveTab::HowToUse, SidebarIcon::HowToUse, t.nav_how_to_use),
                    (ActiveTab::Logs, SidebarIcon::Logs, t.nav_logs),
                ] {
                    let resp = sidebar_item(ui, icon, label, self.active_tab == tab, true);
                    if resp.clicked() {
                        self.active_tab = tab;
                    }
                    ui.add_space(2.0);
                }

                // Bottom utilities
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(4.0);
                    // Language selector (single vertically-centered row matching sidebar items)
                    let popup_id = ui.make_persistent_id("lang_dropdown_popup");
                    let (row_rect, row_resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 32.0),
                        egui::Sense::click(),
                    );
                    if row_resp.hovered() {
                        ui.painter().rect_filled(
                            row_rect,
                            Rounding::same(6.0),
                            Color32::from_rgb(38, 38, 42),
                        );
                    }
                    let flag_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.left() + 20.0, row_rect.center().y),
                        Vec2::new(18.0, 12.0),
                    );
                    draw_flag(ui.painter(), flag_rect, self.lang);
                    ui.painter().text(
                        egui::pos2(row_rect.left() + 36.0, row_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        self.lang.name(),
                        egui::FontId::proportional(14.0),
                        Color32::LIGHT_GRAY,
                    );
                    if row_resp.clicked() {
                        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                    }

                    egui::popup_below_widget(
                        ui,
                        popup_id,
                        &row_resp,
                        PopupCloseBehavior::CloseOnClick,
                        |ui| {
                            ui.set_min_width(130.0);
                            for lang_opt in Language::all() {
                                if lang_opt == self.lang {
                                    continue;
                                }
                                let (opt_rect, opt_resp) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), 26.0),
                                    egui::Sense::click(),
                                );
                                if opt_resp.hovered() {
                                    ui.painter().rect_filled(
                                        opt_rect,
                                        Rounding::same(4.0),
                                        Color32::from_rgb(46, 46, 52),
                                    );
                                }
                                let r = egui::Rect::from_center_size(
                                    egui::pos2(opt_rect.left() + 14.0, opt_rect.center().y),
                                    Vec2::new(18.0, 12.0),
                                );
                                draw_flag(ui.painter(), r, lang_opt);
                                ui.painter().text(
                                    egui::pos2(opt_rect.left() + 30.0, opt_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    lang_opt.name(),
                                    egui::FontId::proportional(14.0),
                                    Color32::LIGHT_GRAY,
                                );
                                if opt_resp.clicked() {
                                    self.set_language(lang_opt);
                                }
                            }
                        },
                    );

                    ui.separator();
                    let min_spin = 0.75_f32;
                    let still_fetching = self.loading_releases;
                    let spin_angle = if let Some(start) = self.refresh_spin_start {
                        let e = start.elapsed().as_secs_f32();
                        if still_fetching || e < min_spin {
                            ui.ctx().request_repaint();
                            e * (std::f32::consts::TAU / min_spin)
                        } else {
                            self.refresh_spin_start = None;
                            0.0
                        }
                    } else if still_fetching {
                        ui.ctx().request_repaint();
                        ui.ctx().input(|i| i.time as f32) * (std::f32::consts::TAU / min_spin)
                    } else {
                        0.0
                    };
                    if sidebar_item_spin(
                        ui,
                        SidebarIcon::Refresh,
                        t.refresh_status,
                        false,
                        !self.busy,
                        spin_angle,
                    )
                    .clicked()
                    {
                        self.refresh_spin_start = Some(std::time::Instant::now());
                        self.refresh_status();
                    }
                    ui.separator();

                    if let Some(info) = self.available_update.clone() {
                        ui.add_space(4.0);
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 32.0),
                            egui::Sense::click(),
                        );
                        let bg = if resp.hovered() {
                            Color32::from_rgb(42, 92, 68)
                        } else {
                            Color32::from_rgb(34, 72, 54)
                        };
                        ui.painter().rect(
                            rect,
                            Rounding::same(6.0),
                            bg,
                            Stroke::new(1.0_f32, Color32::from_rgb(66, 146, 106)),
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 10.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("{} ({})", t.sidebar_update_available, info.latest_tag),
                            egui::FontId::proportional(13.0),
                            Color32::from_rgb(185, 245, 205),
                        );
                        if resp.clicked() {
                            self.show_update_popup = true;
                        }
                    }
                });
            });
    }

    fn render_home(&mut self, ui: &mut egui::Ui, t: &Texts) {
        let has_steam = self.status.steam_dir.is_some();
        let is_locked = self.status.lock_state.is_locked();

        egui::ScrollArea::vertical()
            .id_salt("home_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 1. System Status Card
        card_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t.system_status)
                                .size(16.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.add_space(6.0);
                        match &self.status.lock_state {
                            LockState::Locked => pill_badge(
                                ui,
                                t.locked_pill,
                                Color32::from_rgb(35, 77, 46),
                                Color32::from_rgb(130, 230, 155),
                            ),
                            LockState::PartiallyLocked(_) => pill_badge(
                                ui,
                                t.partial_pill,
                                Color32::from_rgb(90, 66, 24),
                                Color32::from_rgb(250, 212, 120),
                            ),
                            LockState::Unlocked => pill_badge(
                                ui,
                                t.unlocked_pill,
                                Color32::from_rgb(82, 36, 36),
                                Color32::from_rgb(245, 150, 150),
                            ),
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            pill_badge(
                                ui,
                                &self.status.distro.name,
                                Color32::from_rgb(48, 52, 60),
                                Color32::from_rgb(200, 206, 216),
                            );
                            if self.status.distro.in_steamos_gaming_mode {
                                pill_badge(
                                    ui,
                                    "Gaming Mode",
                                    Color32::from_rgb(90, 66, 24),
                                    Color32::from_rgb(250, 212, 120),
                                );
                            }
                            if self.status.is_flatpak {
                                pill_badge(
                                    ui,
                                    "Flatpak",
                                    Color32::from_rgb(44, 58, 86),
                                    Color32::LIGHT_BLUE,
                                );
                            }
                        });
                    });

                    ui.add_space(6.0);
                    let steam_ver_text = match &self.status.current_version {
                        Some(v) => format!(
                            "{}  ({})  [{}]",
                            v.version_id,
                            v.formatted_date,
                            v.channel.suffix()
                        ),
                        None => t.unknown_version.to_string(),
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.steam_client).color(Color32::LIGHT_GRAY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(steam_ver_text);
                        });
                    });
                });

                ui.add_space(10.0);

                let fluent_btn = |ui: &mut egui::Ui,
                                  label: &str,
                                  enabled: bool,
                                  fill: Color32,
                                  stroke: Color32|
                 -> egui::Response {
                    let fg = if enabled {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(130, 134, 140)
                    };
                    let btn = egui::Button::new("")
                        .fill(if enabled {
                            fill
                        } else {
                            Color32::from_rgb(48, 50, 56)
                        })
                        .stroke(Stroke::new(
                            1.0_f32,
                            if enabled {
                                stroke
                            } else {
                                Color32::from_rgb(64, 66, 72)
                            },
                        ))
                        .min_size(Vec2::new(ui.available_width(), 38.0));
                    let resp = ui.add_enabled(enabled, btn);
                    let galley = ui.fonts(|f| {
                        f.layout_no_wrap(
                            label.to_string(),
                            egui::FontId::proportional(14.0),
                            fg,
                        )
                    });
                    ui.painter().galley(
                        egui::pos2(
                            resp.rect.center().x - galley.size().x * 0.5,
                            resp.rect.center().y - galley.size().y * 0.5,
                        ),
                        galley,
                        fg,
                    );
                    resp
                };

                // 2. Rollback Steam Card
        card_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(t.rollback_section_title)
                            .size(16.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(8.0);

                    // 2a. Version Channel Selector
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.channel).strong().color(Color32::WHITE));
                        ui.add_space(6.0);
                        let mut changed_to = None;
                        for (ch, label) in [
                            (ManifestChannel::SteamRt64, t.channel_steamrt64),
                            (ManifestChannel::Ubuntu12, t.channel_ubuntu12),
                        ] {
                            let active = self.selected_channel == Some(ch);
                            let btn = egui::Button::new(RichText::new(label).color(if active {
                                Color32::WHITE
                            } else {
                                Color32::LIGHT_GRAY
                            }))
                            .fill(if active {
                                Color32::from_rgb(42, 88, 142)
                            } else {
                                Color32::from_rgb(44, 45, 50)
                            })
                            .stroke(Stroke::new(
                                1.0_f32,
                                if active {
                                    Color32::from_rgb(76, 136, 206)
                                } else {
                                    Color32::from_rgb(64, 66, 72)
                                },
                            ))
                            .min_size(Vec2::new(0.0, 28.0));
                            if ui.add(btn).clicked() && !active {
                                changed_to = Some(ch);
                            }
                        }
                        if let Some(ch) = changed_to {
                            self.selected_channel = Some(ch);
                            self.channel = ch;
                            self.load_releases(1, true);
                        }
                    });

                    ui.add_space(4.0);
                    let rec_msg = match self.status.current_version.as_ref().map(|v| v.channel) {
                        Some(ManifestChannel::Ubuntu12) => t.tip_ubuntu12_active,
                        _ => t.tip_steamrt64_rec,
                    };
                    ui.label(RichText::new(rec_msg).small().color(Color32::LIGHT_GRAY));

                    // 2b. Available Builds List (unrolls once a channel is selected)
                    if self.selected_channel.is_some() {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(t.available_releases)
                                    .size(14.5)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let reload_btn = egui::Button::new(t.reload_list)
                                    .fill(Color32::from_rgb(44, 45, 50))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(68, 70, 76)))
                                    .min_size(Vec2::new(0.0, 26.0));
                                if ui
                                    .add_enabled(!self.loading_releases, reload_btn)
                                    .clicked()
                                {
                                    self.load_releases(1, true);
                                }
                                ui.add_sized(
                                    [180.0, 26.0],
                                    egui::TextEdit::singleline(&mut self.filter_text)
                                        .hint_text(t.filter_hint),
                                );
                            });
                        });

                        ui.add_space(8.0);
                        egui::ScrollArea::vertical()
                            .id_salt("release_list_scroll")
                            .max_height(165.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 5.0;
                                let query = self.filter_text.to_lowercase();

                                for (i, entry) in self.releases.iter().enumerate() {
                                    if !query.is_empty()
                                        && !entry.date_display.to_lowercase().contains(&query)
                                        && !entry.short_sha.to_lowercase().contains(&query)
                                        && !entry.message.to_lowercase().contains(&query)
                                    {
                                        continue;
                                    }
                                    let selected = self.selected_idx == Some(i);
                                    let short_title = if i == 0 {
                                        format!("{}   {}", entry.date_display, t.latest_badge)
                                    } else {
                                        entry.date_display.clone()
                                    };
                                    let hover_details = format!(
                                        "{}: {}\n{}: {}\n{}: {}\n{}: {}",
                                        t.date_label,
                                        entry.date_display,
                                        t.channel,
                                        entry.channel.suffix(),
                                        t.commit_sha_label,
                                        entry.sha,
                                        t.commit_msg_label,
                                        entry.message
                                    );

                                    ui.horizontal(|ui| {
                                        let info_w = 56.0;
                                        let btn_w = (ui.available_width() - info_w - 8.0).max(120.0);
                                        let row_btn = egui::Button::new(
                                            RichText::new(short_title).color(if selected {
                                                Color32::WHITE
                                            } else {
                                                Color32::LIGHT_GRAY
                                            }),
                                        )
                                        .fill(if selected {
                                            Color32::from_rgb(42, 88, 142)
                                        } else {
                                            Color32::from_rgb(38, 39, 43)
                                        })
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            if selected {
                                                Color32::from_rgb(76, 136, 206)
                                            } else {
                                                Color32::from_rgb(54, 55, 60)
                                            },
                                        ))
                                        .min_size(Vec2::new(btn_w, 30.0));
                                        if ui
                                            .add(row_btn)
                                            .on_hover_text(&hover_details)
                                            .clicked()
                                            && !self.busy
                                        {
                                            self.selected_idx = Some(i);
                                        }

                                        let info_btn = egui::Button::new(t.info_btn)
                                            .fill(Color32::from_rgb(44, 45, 50))
                                            .stroke(Stroke::new(
                                                1.0_f32,
                                                Color32::from_rgb(64, 66, 72),
                                            ))
                                            .min_size(Vec2::new(info_w, 30.0));
                                        if ui.add(info_btn).on_hover_text(t.info_hover).clicked() {
                                            self.details_entry = Some(entry.clone());
                                        }
                                    });
                                }

                                ui.add_space(4.0);
                                if self.loading_releases {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(t.loading_releases);
                                    });
                                } else if self.has_more {
                                    let more_btn = egui::Button::new(t.load_older)
                                        .fill(Color32::from_rgb(44, 45, 50))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            Color32::from_rgb(68, 70, 76),
                                        ))
                                        .min_size(Vec2::new(ui.available_width(), 28.0));
                                    if ui.add(more_btn).clicked() {
                                        let next_page = self.current_page + 1;
                                        self.load_releases(next_page, false);
                                    }
                                }
                            });
                    }

                    ui.add_space(10.0);

                    // 2c. Rollback & Lock to Selected Version | Lock Only
                    let can_rollback = has_steam
                        && self.selected_channel.is_some()
                        && self.selected_idx.is_some()
                        && !self.busy;
                    let can_lock = has_steam && !self.busy;

                    ui.columns(2, |cols| {
                        cols[0].vertical(|ui| {
                            if fluent_btn(
                                ui,
                                t.rollback_and_lock,
                                can_rollback,
                                Color32::from_rgb(42, 88, 142),
                                Color32::from_rgb(76, 136, 206),
                            )
                            .clicked()
                            {
                                self.start_rollback();
                            }
                            ui.add_space(4.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(t.rollback_and_lock_desc)
                                        .small()
                                        .color(Color32::LIGHT_GRAY),
                                )
                                .wrap(),
                            );
                        });

                        cols[1].vertical(|ui| {
                            if fluent_btn(
                                ui,
                                t.lock_only,
                                can_lock,
                                Color32::from_rgb(58, 62, 72),
                                Color32::from_rgb(92, 98, 112),
                            )
                            .clicked()
                            {
                                self.start_lock_only();
                            }
                            ui.add_space(4.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(t.lock_only_desc)
                                        .small()
                                        .color(Color32::LIGHT_GRAY),
                                )
                                .wrap(),
                            );
                        });
                    });
                });

                ui.add_space(10.0);

                // 3. Unlock Steam Card
        card_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(t.unlock_section_title)
                            .size(16.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(8.0);

                    let can_unlock = has_steam && !self.busy && is_locked;
                    ui.columns(2, |cols| {
                        cols[0].vertical(|ui| {
                            if fluent_btn(
                                ui,
                                t.unlock_and_update,
                                can_unlock,
                                Color32::from_rgb(38, 92, 66),
                                Color32::from_rgb(66, 146, 106),
                            )
                            .clicked()
                            {
                                self.start_unlock_and_update();
                            }
                            ui.add_space(4.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(t.unlock_and_update_desc)
                                        .small()
                                        .color(Color32::LIGHT_GRAY),
                                )
                                .wrap(),
                            );
                        });

                        cols[1].vertical(|ui| {
                            if fluent_btn(
                                ui,
                                t.unlock_updates,
                                can_unlock,
                                Color32::from_rgb(58, 62, 72),
                                Color32::from_rgb(92, 98, 112),
                            )
                            .clicked()
                            {
                                self.start_unlock();
                            }
                            ui.add_space(4.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(t.unlock_only_desc)
                                        .small()
                                        .color(Color32::LIGHT_GRAY),
                                )
                                .wrap(),
                            );
                        });
                    });

                    if self.busy {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new(t.working).strong());
                            if let Some(last) = self.logs.last() {
                                ui.label(
                                    RichText::new(format!("— {last}")).color(Color32::LIGHT_GRAY),
                                );
                            }
                        });
                    }
                });

                ui.add_space(8.0);
                let muted = Color32::from_rgb(135, 138, 145);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(format!("v{CURRENT_VERSION}"))
                            .size(11.5)
                            .color(muted),
                    );
                    ui.horizontal(|ui| {
                        let approx_w = 138.0;
                        let pad = ((ui.available_width() - approx_w) * 0.5).max(0.0);
                        ui.add_space(pad);
                        ui.hyperlink_to(
                            RichText::new("GitHub")
                                .size(11.5)
                                .color(Color32::from_rgb(145, 175, 220)),
                            "https://github.com/foodiemonster/SteamRollbackrLite",
                        );
                        ui.label(
                            RichText::new("•  by foodiemonster")
                                .size(11.5)
                                .color(muted),
                        );
                    });
                });
            });
    }

    fn render_tools(&mut self, ui: &mut egui::Ui, t: &Texts) {
        ui.label(
            RichText::new(t.tools_title)
                .size(20.0)
                .strong()
                .color(Color32::WHITE),
        );
        ui.label(RichText::new(t.tools_subtitle).color(Color32::LIGHT_GRAY));
        ui.add_space(12.0);

        let has_steam = self.status.steam_dir.is_some();
        let libcrypto_installed = self
            .status
            .steam_dir
            .as_ref()
            .is_some_and(|d| d.join("ubuntu12_32/libcrypto.so.3").is_file());

        let bold_font = egui::FontId::new(15.5, egui::FontFamily::Name("fluent_bold".into()));
        let green_bg = Color32::from_rgb(35, 77, 46);
        let green_fg = Color32::from_rgb(130, 230, 155);
        let red_bg = Color32::from_rgb(82, 36, 36);
        let red_fg = Color32::from_rgb(245, 150, 150);

        let tool_btn = |ui: &mut egui::Ui,
                        label: &str,
                        enabled: bool,
                        fill: Color32,
                        stroke: Color32|
         -> egui::Response {
            let fg = if enabled {
                Color32::WHITE
            } else {
                Color32::from_rgb(130, 134, 140)
            };
            let btn = egui::Button::new("")
                .fill(if enabled {
                    fill
                } else {
                    Color32::from_rgb(48, 50, 56)
                })
                .stroke(Stroke::new(
                    1.0_f32,
                    if enabled {
                        stroke
                    } else {
                        Color32::from_rgb(64, 66, 72)
                    },
                ))
                .min_size(Vec2::new(ui.available_width(), 34.0));
            let resp = ui.add_enabled(enabled, btn);
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(label.to_string(), egui::FontId::proportional(14.0), fg)
            });
            ui.painter().galley(
                egui::pos2(
                    resp.rect.center().x - galley.size().x * 0.5,
                    resp.rect.center().y - galley.size().y * 0.5,
                ),
                galley,
                fg,
            );
            resp
        };

        if self.busy {
            egui::Frame::none()
                .fill(Color32::from_rgb(36, 52, 74))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(76, 136, 206)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(t.working).strong().color(Color32::WHITE));
                        if let Some(last) = self.logs.last() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("— {last}"))
                                        .color(Color32::from_rgb(200, 220, 245)),
                                )
                                .truncate(),
                            );
                        }
                    });
                });
            ui.add_space(10.0);
        }

        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t.libcrypto_title)
                        .font(bold_font.clone())
                        .color(Color32::WHITE),
                );
                ui.add_space(6.0);
                if libcrypto_installed {
                    pill_badge(ui, t.installed_pill, green_bg, green_fg);
                } else {
                    pill_badge(ui, t.missing_pill, red_bg, red_fg);
                }
            });
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    RichText::new(t.libcrypto_desc).color(Color32::LIGHT_GRAY),
                )
                .wrap(),
            );
            ui.add_space(10.0);
            ui.columns(2, |cols| {
                let can_install = has_steam && !self.busy && !libcrypto_installed;
                let can_uninstall = has_steam && !self.busy && libcrypto_installed;
                if tool_btn(
                    &mut cols[0],
                    t.install_btn,
                    can_install,
                    Color32::from_rgb(42, 88, 142),
                    Color32::from_rgb(76, 136, 206),
                )
                .clicked()
                {
                    self.start_install_libcrypto3();
                }
                if tool_btn(
                    &mut cols[1],
                    t.uninstall_btn,
                    can_uninstall,
                    Color32::from_rgb(122, 40, 40),
                    Color32::from_rgb(184, 72, 72),
                )
                .clicked()
                {
                    self.start_uninstall_libcrypto3();
                }
            });
        });
    }

    fn render_options(&mut self, ui: &mut egui::Ui, t: &Texts) {
        ui.label(
            RichText::new(t.options_title)
                .size(20.0)
                .strong()
                .color(Color32::WHITE),
        );
        ui.label(RichText::new(t.options_subtitle).color(Color32::LIGHT_GRAY));
        ui.add_space(12.0);

        let install_kind = detect_install_kind();
        let is_system_pkg = install_kind == InstallKind::SystemPackage;
        let is_user_installed = is_app_installed_user_local();

        let bold_font = egui::FontId::new(15.5, egui::FontFamily::Name("fluent_bold".into()));
        let green_bg = Color32::from_rgb(35, 77, 46);
        let green_fg = Color32::from_rgb(130, 230, 155);
        let blue_bg = Color32::from_rgb(44, 58, 86);

        let opt_btn = |ui: &mut egui::Ui,
                       label: &str,
                       enabled: bool,
                       fill: Color32,
                       stroke: Color32|
         -> egui::Response {
            let fg = if enabled {
                Color32::WHITE
            } else {
                Color32::from_rgb(130, 134, 140)
            };
            let btn = egui::Button::new("")
                .fill(if enabled {
                    fill
                } else {
                    Color32::from_rgb(48, 50, 56)
                })
                .stroke(Stroke::new(
                    1.0_f32,
                    if enabled {
                        stroke
                    } else {
                        Color32::from_rgb(64, 66, 72)
                    },
                ))
                .min_size(Vec2::new(ui.available_width(), 34.0));
            let resp = ui.add_enabled(enabled, btn);
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(label.to_string(), egui::FontId::proportional(14.0), fg)
            });
            ui.painter().galley(
                egui::pos2(
                    resp.rect.center().x - galley.size().x * 0.5,
                    resp.rect.center().y - galley.size().y * 0.5,
                ),
                galley,
                fg,
            );
            resp
        };

        if self.busy {
            egui::Frame::none()
                .fill(Color32::from_rgb(36, 52, 74))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(76, 136, 206)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(t.working).strong().color(Color32::WHITE));
                        if let Some(last) = self.logs.last() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("— {last}"))
                                        .color(Color32::from_rgb(200, 220, 245)),
                                )
                                .truncate(),
                            );
                        }
                    });
                });
            ui.add_space(10.0);
        }

        // 1. Steam Path Card
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t.steam_path)
                        .font(bold_font.clone())
                        .color(Color32::WHITE),
                );
                ui.add_space(6.0);
                if self.status.steam_dir.is_none() {
                    pill_badge(
                        ui,
                        t.steam_not_found,
                        Color32::from_rgb(82, 36, 36),
                        Color32::from_rgb(245, 150, 150),
                    );
                } else if self.steam_path_override.is_some() {
                    pill_badge(ui, t.custom_pill, blue_bg, Color32::LIGHT_BLUE);
                } else {
                    pill_badge(ui, t.auto_pill, green_bg, green_fg);
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let reset_w = 96.0;
                let apply_w = 76.0;
                let input_w = (ui.available_width() - reset_w - apply_w - 16.0).max(140.0);
                let path_edit = egui::TextEdit::singleline(&mut self.steam_path_input)
                    .hint_text("/path/to/Steam")
                    .font(egui::TextStyle::Monospace)
                    .desired_width(input_w);
                let resp = ui.add_sized([input_w, 28.0], path_edit);

                let trimmed = self.steam_path_input.trim();
                let can_apply = !self.busy && !trimmed.is_empty();
                let apply_btn =
                    egui::Button::new(RichText::new(t.apply_btn).color(if can_apply {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(130, 134, 140)
                    }))
                    .fill(if can_apply {
                        Color32::from_rgb(42, 88, 142)
                    } else {
                        Color32::from_rgb(48, 50, 56)
                    })
                    .stroke(Stroke::new(
                        1.0_f32,
                        if can_apply {
                            Color32::from_rgb(76, 136, 206)
                        } else {
                            Color32::from_rgb(64, 66, 72)
                        },
                    ))
                    .min_size(Vec2::new(apply_w, 28.0));

                let enter_pressed =
                    resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.add_enabled(can_apply, apply_btn).clicked() || (enter_pressed && can_apply) {
                    self.steam_path_override = Some(PathBuf::from(trimmed));
                    self.refresh_status();
                }

                let can_reset = !self.busy && self.steam_path_override.is_some();
                let reset_btn = egui::Button::new(t.reset_auto_btn)
                    .fill(Color32::from_rgb(44, 45, 50))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(68, 70, 76)))
                    .min_size(Vec2::new(reset_w, 28.0));
                if ui.add_enabled(can_reset, reset_btn).clicked() {
                    self.steam_path_override = None;
                    self.refresh_status();
                }
            });
        });

        ui.add_space(10.0);

        // 2. Install SteamRollbackrLite Card
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t.app_install_title)
                        .font(bold_font)
                        .color(Color32::WHITE),
                );
                ui.add_space(6.0);
                if is_system_pkg {
                    pill_badge(ui, t.system_pkg_pill, green_bg, green_fg);
                } else if is_user_installed {
                    pill_badge(ui, t.installed_pill, green_bg, green_fg);
                } else {
                    pill_badge(ui, t.portable_pill, blue_bg, Color32::LIGHT_BLUE);
                }
            });
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    RichText::new(t.app_install_desc).color(Color32::LIGHT_GRAY),
                )
                .wrap(),
            );
            ui.add_space(10.0);
            ui.columns(2, |cols| {
                let can_install = !self.busy && !is_system_pkg;
                let can_uninstall = !self.busy && !is_system_pkg && is_user_installed;
                if opt_btn(
                    &mut cols[0],
                    t.install_btn,
                    can_install,
                    Color32::from_rgb(42, 88, 142),
                    Color32::from_rgb(76, 136, 206),
                )
                .clicked()
                {
                    self.start_install_app_user_local();
                }
                if opt_btn(
                    &mut cols[1],
                    t.uninstall_btn,
                    can_uninstall,
                    Color32::from_rgb(122, 40, 40),
                    Color32::from_rgb(184, 72, 72),
                )
                .clicked()
                {
                    self.start_uninstall_app_user_local();
                }
            });
        });
    }

    fn render_how_to_use(&self, ui: &mut egui::Ui, t: &Texts) {
        ui.label(
            RichText::new(t.how_to_use_title)
                .size(20.0)
                .strong()
                .color(Color32::WHITE),
        );
        ui.label(RichText::new(t.how_to_use_subtitle).color(Color32::LIGHT_GRAY));
        ui.add_space(12.0);

        let bold_font = egui::FontId::new(14.5, egui::FontFamily::Name("fluent_bold".into()));

        egui::ScrollArea::vertical()
            .id_salt("how_to_use_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let steps = [
                    (t.step1_title, t.step1_desc),
                    (t.step2_title, t.step2_desc),
                    (t.step3_title, t.step3_desc),
                    (t.step4_title, t.step4_desc),
                    (t.step5_title, t.step5_desc),
                ];
                for (title, desc) in steps {
                    card_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(title)
                                .font(bold_font.clone())
                                .color(Color32::WHITE),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(RichText::new(desc).color(Color32::LIGHT_GRAY))
                                .wrap(),
                        );
                    });
                    ui.add_space(8.0);
                }
            });
    }

    fn render_logs(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, t: &Texts) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t.logs_title)
                    .size(20.0)
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t.clear_logs).clicked() {
                    self.initial_ready_only = false;
                    self.logs.clear();
                }
                if ui.button(t.copy_logs).clicked() {
                    ctx.copy_text(self.logs.join("\n"));
                }
            });
        });
        ui.add_space(10.0);

        card_frame().show(ui, |ui| {
                egui::ScrollArea::vertical()
                .id_salt("activity_log_scroll")
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for line in &self.logs {
                        ui.monospace(line);
                    }
                });
        });
    }
}

impl eframe::App for SteamRollbackrLiteApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        if self.busy || self.loading_releases {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let t = texts(self.lang);

        self.render_sidebar(ctx, &t);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(35, 35, 38))
                    .inner_margin(Margin::same(20.0)),
            )
            .show(ctx, |ui| match self.active_tab {
                ActiveTab::Home => self.render_home(ui, &t),
                ActiveTab::Tools => self.render_tools(ui, &t),
                ActiveTab::Options => self.render_options(ui, &t),
                ActiveTab::HowToUse => self.render_how_to_use(ui, &t),
                ActiveTab::Logs => self.render_logs(ctx, ui, &t),
            });

        if let Some(entry) = self.details_entry.clone() {
            let mut open = true;
            egui::Window::new(t.release_details_title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.date_label).strong());
                        ui.label(&entry.date_display);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.channel).strong());
                        ui.label(entry.channel.suffix());
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.commit_sha_label).strong());
                        ui.monospace(&entry.sha);
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new(t.commit_msg_label).strong());
                    ui.label(&entry.message);
                    ui.add_space(6.0);
                    let url = format!(
                        "https://github.com/SteamTracking/SteamTracking/commit/{}",
                        entry.sha
                    );
                    ui.hyperlink_to(t.view_on_github, url);
                });
            if !open {
                self.details_entry = None;
            }
        }

        if self.show_update_popup {
            if let Some(info) = self.available_update.clone() {
                let mut open = true;
                egui::Window::new(t.update_available_title)
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        let popup_w = 340.0;
                        ui.set_width(popup_w);
                        ui.add_space(4.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(t.update_available_msg).color(Color32::WHITE));
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!(
                                    "v{}  ->  {}",
                                    CURRENT_VERSION, info.latest_tag
                                ))
                                .font(egui::FontId::new(
                                    14.0,
                                    egui::FontFamily::Name("fluent_bold".into()),
                                ))
                                .color(Color32::from_rgb(130, 230, 155)),
                            );
                        });
                        ui.add_space(12.0);

                        if self.updating_app {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(RichText::new(t.updating_app).color(Color32::LIGHT_GRAY));
                            });
                            ui.add_space(4.0);
                        } else {
                            let side_margin = 6.0;
                            let gap = 8.0;
                            let btn_w = (popup_w - side_margin * 2.0 - gap) * 0.5;
                            let primary_label = match detect_install_kind() {
                                InstallKind::SystemPackage => t.update_open_page_btn,
                                _ => t.update_now_btn,
                            };

                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.add_space(side_margin);
                                let update_btn = egui::Button::new(
                                    RichText::new(primary_label).color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(38, 92, 66))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(66, 146, 106)))
                                .min_size(Vec2::new(btn_w, 32.0));
                                if ui.add(update_btn).clicked() {
                                    self.start_app_update();
                                }

                                ui.add_space(gap);

                                let later_btn = egui::Button::new(
                                    RichText::new(t.later_btn).color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(122, 40, 40))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(184, 72, 72)))
                                .min_size(Vec2::new(btn_w, 32.0));
                                if ui.add(later_btn).clicked() {
                                    self.show_update_popup = false;
                                }
                            });
                            ui.add_space(4.0);
                        }
                    });
                if !open && !self.updating_app {
                    self.show_update_popup = false;
                }
            }
        }
    }
}
