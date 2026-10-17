//! The phone layout: the same panels, the same engine, a different shell.
//!
//! The desktop shell (menu bar, tools bar, a tree of docked panels) needs a large window. On a
//! phone (a short side under [`PHONE_MAX_SHORT_SIDE`] points, or an Android build) the app shows
//! this one instead, built from the same `panel_body` calls so every panel keeps working:
//!
//! ```text
//! portrait                      landscape
//! ┌──────────────────────┐      ┌───────────────┬─────────────┐
//! │ ☰  Project.ecproj  ↶ ↷ ⛶ ▶│      │ top bar                         │
//! ├──────────────────────┤      ├───────────────┬─────────────┤
//! │ Composition viewer   │      │ Composition   │ active      │
//! ├──── drag to resize ──┤      │ viewer        │ panel       │
//! │ active panel         │      │               │             │
//! ├──────────────────────┤      ├───────────────┴─────────────┤
//! │ Project Timeline … ⋯ │      │ bottom navigation           │
//! └──────────────────────┘      └─────────────────────────────┘
//! ```
//!
//! - The ☰ drawer holds every menu of the desktop menu bar (File, Edit, Composition, Layer, Effect,
//!   Animation, View, Window, Help), so every command stays reachable.
//! - The bottom bar switches the lower panel; ⋯ More lists all the other panels.
//! - The line between the viewer and the lower panel is a handle: drag it to give either more room.
//! - ⛶ gives the whole screen to the viewer.
//! - Controls get larger touch targets ([`touch_style`]).

use std::sync::atomic::{AtomicBool, Ordering};

use effectcraft_engine::menus::MenuNode;
use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::dock::PanelKind;
use crate::icons::{self, Icon};
use crate::theme::Tokens;
use crate::{EffectcraftApp, menus, panels, widgets};

/// A window whose shorter side is under this many points gets the phone layout (the desktop
/// window never gets smaller than 960 x 600).
pub const PHONE_MAX_SHORT_SIDE: f32 = 560.0;

const BAR_H: f32 = 52.0;
const NAV_H: f32 = 58.0;
const BTN: f32 = 44.0;
const GRIP: f32 = 24.0;
/// How far the toast and the Importing card rise above the bottom bar.
pub(crate) const NAV_LIFT: f32 = NAV_H + 8.0;

static ACTIVE: AtomicBool = AtomicBool::new(false);

/// The phone layout is showing (read by the theme, which then adds touch sizes).
pub fn touch_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// The phone layout's own state (not saved with the workspace).
pub struct State {
    /// `Some(false)` keeps the desktop layout on a small screen, `Some(true)` forces the phone
    /// layout on a big one, `None` decides by the window size.
    pub force: Option<bool>,
    /// The panel shown below (or beside) the viewer.
    tab: PanelKind,
    /// The panel that had the focus last frame: a panel focused from elsewhere (an effect applied
    /// brings up Effect Controls) becomes the tab.
    seen_focus: PanelKind,
    /// Share of the area the viewer takes, in portrait and in landscape.
    frac_portrait: f32,
    frac_landscape: f32,
    /// The viewer fills the screen.
    viewer_full: bool,
    menu_open: bool,
    more_open: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            force: None,
            tab: PanelKind::Timeline,
            seen_focus: PanelKind::Timeline,
            frac_portrait: 0.42,
            frac_landscape: 0.5,
            viewer_full: false,
            menu_open: false,
            more_open: false,
        }
    }
}

/// Whether the phone layout applies to a window of this size.
pub(crate) fn detect(app: &EffectcraftApp, full: Rect) -> bool {
    match app.mobile.force {
        Some(on) => on,
        None => cfg!(target_os = "android") || full.width().min(full.height()) < PHONE_MAX_SHORT_SIDE,
    }
}

/// Remember whether the phone layout is showing. Returns true when that changed (the caller then
/// re-applies the theme so the touch sizes come or go).
pub(crate) fn set_active(on: bool) -> bool {
    ACTIVE.swap(on, Ordering::Relaxed) != on
}

/// Bigger controls for fingers: taller rows, wider padding, larger text, thicker scroll bars.
/// Called from the end of `theme::apply_visuals` while the phone layout shows.
pub(crate) fn touch_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|s| {
        s.spacing.interact_size = vec2(40.0, 36.0);
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 6.0);
        s.spacing.icon_width = 22.0;
        s.spacing.icon_width_inner = 12.0;
        s.spacing.scroll.bar_width = 12.0;
        s.text_styles.insert(egui::TextStyle::Body, Tokens::ui(14.0));
        s.text_styles.insert(egui::TextStyle::Button, Tokens::ui(14.0));
        s.text_styles.insert(egui::TextStyle::Small, Tokens::ui(12.0));
    });
}

/// Draw the whole phone shell inside `full`.
pub(crate) fn show(app: &mut EffectcraftApp, ui: &mut egui::Ui, full: Rect) {
    // A panel focused from elsewhere (not by a touch here) becomes the lower panel.
    let focused = app.ui.focused;
    if focused != app.mobile.seen_focus {
        app.mobile.seen_focus = focused;
        if is_tab_panel(focused) {
            app.mobile.tab = focused;
        }
    }
    let top = Rect::from_min_size(full.min, vec2(full.width(), BAR_H));
    let nav = Rect::from_min_max(pos2(full.min.x, full.max.y - NAV_H), full.max);
    let body = Rect::from_min_max(pos2(full.min.x, top.max.y), pos2(full.max.x, nav.min.y));
    top_bar(app, ui, top);
    body_area(app, ui, body);
    bottom_nav(app, ui, nav);
    if app.mobile.more_open {
        more_sheet(app, ui, full, nav);
    }
    if app.mobile.menu_open {
        menu_drawer(app, ui, full);
    }
}

/// On a small screen that was switched to the desktop layout: a small button to come back.
pub(crate) fn desktop_escape(app: &mut EffectcraftApp, ui: &mut egui::Ui, full: Rect) {
    if app.mobile.force != Some(false) || full.width().min(full.height()) >= PHONE_MAX_SHORT_SIDE {
        return;
    }
    let ctx = ui.ctx().clone();
    let mut back = false;
    egui::Area::new(egui::Id::new("m-escape")).order(egui::Order::Foreground).fixed_pos(pos2(full.max.x - 150.0, full.max.y - 56.0)).show(&ctx, |ui| {
        if ui.add_sized([140.0, 44.0], egui::Button::new("Phone layout")).clicked() {
            back = true;
        }
    });
    if back {
        app.mobile.force = None;
    }
}

fn is_tab_panel(p: PanelKind) -> bool {
    !matches!(p, PanelKind::Composition | PanelKind::Viewer(_))
}

// ------------------------------------------------------------------------ top bar

fn top_bar(app: &mut EffectcraftApp, ui: &mut egui::Ui, r: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    let p = ui.painter().clone();
    p.rect_filled(r, 0.0, t.header_bg);
    p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, t.separator));
    let y = r.min.y + (r.height() - BTN) / 2.0;

    // Left: the menu drawer.
    let menu = Rect::from_min_size(pos2(r.min.x + 4.0, y), vec2(BTN, BTN));
    let resp = widgets::icon_button(ui, menu, Icon::Hamburger, app.mobile.menu_open, &t, egui::Id::new("m-menu"));
    app.auto.add_response("mobile.menu", &resp, "Menu");
    if resp.clicked() {
        app.mobile.menu_open = !app.mobile.menu_open;
        app.mobile.more_open = false;
    }

    // Right, from the edge in: play, viewer full screen, redo, undo.
    let slot = |n: f32| Rect::from_min_size(pos2(r.max.x - 4.0 - (n + 1.0) * (BTN + 2.0) + 2.0, y), vec2(BTN, BTN));
    let playing = app.playback.playing;
    let play = widgets::icon_button(ui, slot(0.0), if playing { Icon::Pause } else { Icon::Play }, playing, &t, egui::Id::new("m-play"));
    app.auto.add_response("mobile.play", &play, "Play / Pause");
    if play.clicked() {
        let now = ui.input(|i| i.time);
        app.toggle_play(now);
    }
    let full_resp = widgets::icon_button(ui, slot(1.0), Icon::Fullscreen, app.mobile.viewer_full, &t, egui::Id::new("m-viewer-full"));
    app.auto.add_response("mobile.viewerFull", &full_resp, "Viewer full screen");
    if full_resp.clicked() {
        app.mobile.viewer_full = !app.mobile.viewer_full;
    }
    for (n, redo) in [(2.0, true), (3.0, false)] {
        let rect = slot(n);
        let resp = ui.interact(rect, egui::Id::new(("m-undo", redo)), Sense::click());
        if resp.is_pointer_button_down_on() {
            p.rect_filled(rect, 4.0, t.pressed);
        }
        paint_undo(&p, rect, redo, t.icon);
        app.auto.add_response(if redo { "mobile.redo" } else { "mobile.undo" }, &resp, if redo { "Redo" } else { "Undo" });
        if resp.clicked()
            && let Err(e) = menus::invoke(app, &ctx, if redo { "edit.redo" } else { "edit.undo" }, json!({}))
        {
            app.ui.status = e;
        }
    }

    // Middle: the project's name.
    let left = menu.max.x + 6.0;
    let right = slot(3.0).min.x - 6.0;
    if right > left {
        let name = panels::unsaved::project_name(app);
        let name = name.strip_suffix(".ecproj").unwrap_or(&name).to_string();
        let name = if app.session.is_dirty() { format!("{name} *") } else { name };
        let clip = Rect::from_min_max(pos2(left, r.min.y), pos2(right, r.max.y));
        p.with_clip_rect(clip).text(pos2(left, r.center().y), Align2::LEFT_CENTER, name, Tokens::semibold(15.0), t.text);
    }
}

/// An undo (counter-clockwise) or redo (mirrored) arrow: an arc with an arrow head.
fn paint_undo(p: &egui::Painter, rect: Rect, redo: bool, col: Color32) {
    let s = if redo { -1.0 } else { 1.0 };
    let c = rect.center() + vec2(0.0, rect.height() * 0.05);
    let rad = rect.width() * 0.24;
    let at = |deg: f32| {
        let a = deg.to_radians();
        pos2(c.x + s * rad * a.cos(), c.y - rad * a.sin())
    };
    let pts: Vec<egui::Pos2> = (0..=16).map(|i| at(i as f32 * 200.0 / 16.0)).collect();
    let stroke = Stroke::new(2.0, col);
    p.add(egui::Shape::line(pts, stroke));
    // The head at the end of the arc, pointing along it.
    let end = at(200.0);
    let back = at(180.0);
    let dir = (end - back).normalized();
    let normal = vec2(-dir.y, dir.x);
    let len = rad * 0.7;
    p.line_segment([end, end - dir * len + normal * len * 0.6], stroke);
    p.line_segment([end, end - dir * len - normal * len * 0.6], stroke);
}

// ------------------------------------------------------------------------ body

fn body_area(app: &mut EffectcraftApp, ui: &mut egui::Ui, body: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    if app.ui.start_screen {
        // Home covers the whole area, as on the desktop.
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).id_salt("home"));
        child.set_clip_rect(body.intersect(ui.clip_rect()));
        panels::home::show(app, &mut child, body);
        return;
    }
    let viewer = panels::viewers::active_panel(app);
    let tab = app.mobile.tab;
    let landscape = body.width() > body.height() * 1.25;

    // Split the area: viewer, a drag handle, the lower panel.
    let (viewer_rect, handle, lower) = if app.mobile.viewer_full {
        (body, None, None)
    } else if landscape {
        let x = (body.min.x + body.width() * app.mobile.frac_landscape).round();
        let handle = Rect::from_min_max(pos2(x - GRIP / 2.0, body.min.y), pos2(x + GRIP / 2.0, body.max.y));
        (Rect::from_min_max(body.min, pos2(handle.min.x, body.max.y)), Some(handle), Some(Rect::from_min_max(pos2(handle.max.x, body.min.y), body.max)))
    } else {
        let y = (body.min.y + body.height() * app.mobile.frac_portrait).round();
        let handle = Rect::from_min_max(pos2(body.min.x, y - GRIP / 2.0), pos2(body.max.x, y + GRIP / 2.0));
        (Rect::from_min_max(body.min, pos2(body.max.x, handle.min.y)), Some(handle), Some(Rect::from_min_max(pos2(body.min.x, handle.max.y), body.max)))
    };

    if let Some(h) = handle {
        let p = ui.painter().clone();
        p.rect_filled(h, 0.0, t.header_bg);
        let resp = ui.interact(h.expand2(if landscape { vec2(8.0, 0.0) } else { vec2(0.0, 8.0) }), egui::Id::new("m-split"), Sense::drag());
        let (grip_size, delta, extent) = if landscape { (vec2(4.0, 40.0), resp.drag_delta().x, body.width()) } else { (vec2(40.0, 4.0), resp.drag_delta().y, body.height()) };
        if extent > 1.0 && delta != 0.0 {
            let f = if landscape { &mut app.mobile.frac_landscape } else { &mut app.mobile.frac_portrait };
            *f = (*f + delta / extent).clamp(0.18, 0.8);
        }
        let col = if resp.dragged() { t.accent } else { t.text_faint };
        p.rect_filled(Rect::from_center_size(h.center(), grip_size), 2.0, col);
        app.auto.add_response("mobile.splitter", &resp, "Resize viewer");
    }

    app.dock_rects = std::iter::once((viewer, viewer_rect)).chain(lower.map(|r| (tab, r))).collect();
    app.panel_body(ui, viewer, viewer_rect);
    if let Some(r) = lower {
        app.panel_body(ui, tab, r);
    }

    // A touch in a panel gives it the focus (the keyboard shortcuts and the panel-specific
    // commands go to the focused panel).
    if ui.input(|i| i.pointer.any_pressed())
        && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
    {
        let hit = if viewer_rect.contains(pos) { Some(viewer) } else { lower.filter(|r| r.contains(pos)).map(|_| tab) };
        if let Some(p) = hit {
            app.ui.focused = p;
            app.mobile.seen_focus = p;
            panels::viewer_text::panel_clicked(app, &ctx, p);
        }
    }
    let stroke_rect = if app.ui.focused == viewer { Some(viewer_rect) } else { lower.filter(|_| app.ui.focused == tab) };
    if let Some(r) = stroke_rect {
        ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t.focus), StrokeKind::Inside);
    }
}

// ------------------------------------------------------------------------ bottom bar

fn nav_items() -> [(PanelKind, Icon, &'static str); 4] {
    [
        (PanelKind::Project, Icon::Folder, "Project"),
        (PanelKind::Timeline, Icon::Keyframe, "Timeline"),
        (PanelKind::EffectControls, Icon::Fx, "Controls"),
        (PanelKind::EffectsPresets, Icon::Sparkle, "Effects"),
    ]
}

fn bottom_nav(app: &mut EffectcraftApp, ui: &mut egui::Ui, nav: Rect) {
    let t = app.tokens;
    let p = ui.painter().clone();
    p.rect_filled(nav, 0.0, t.header_bg);
    p.line_segment([nav.left_top(), nav.right_top()], Stroke::new(1.0, t.separator));
    let items = nav_items();
    let cells = items.len() + 1;
    let w = nav.width() / cells as f32;
    let current = app.mobile.tab;
    let in_bar = items.iter().any(|(k, _, _)| *k == current);
    for i in 0..cells {
        let cell = Rect::from_min_size(pos2(nav.min.x + w * i as f32, nav.min.y), vec2(w, nav.height()));
        let (kind, icon, label) = match items.get(i) {
            Some((k, icon, label)) => (Some(*k), *icon, *label),
            None => (None, Icon::Grid, "More"),
        };
        let active = match kind {
            Some(k) => k == current && !app.mobile.more_open,
            None => app.mobile.more_open || !in_bar,
        };
        let resp = ui.interact(cell, egui::Id::new(("m-nav", i)), Sense::click());
        if resp.is_pointer_button_down_on() {
            p.rect_filled(cell, 0.0, t.pressed);
        }
        let col = if active { t.accent } else { t.icon };
        if active {
            p.rect_filled(Rect::from_min_size(cell.min, vec2(cell.width(), 3.0)), 0.0, t.accent);
        }
        icons::paint(&p, Rect::from_center_size(pos2(cell.center().x, cell.min.y + 21.0), vec2(26.0, 26.0)), icon, col);
        p.text(pos2(cell.center().x, cell.max.y - 10.0), Align2::CENTER_CENTER, crate::i18n::tr(label), Tokens::ui(11.0), if active { t.text } else { t.text_dim });
        app.auto.add_response(&format!("mobile.nav.{label}"), &resp, label);
        if resp.clicked() {
            match kind {
                Some(k) => {
                    app.mobile.more_open = false;
                    app.mobile.menu_open = false;
                    select_tab(app, k);
                }
                None => {
                    app.mobile.more_open = !app.mobile.more_open;
                    app.mobile.menu_open = false;
                }
            }
        }
    }
}

fn select_tab(app: &mut EffectcraftApp, p: PanelKind) {
    app.mobile.tab = p;
    app.mobile.seen_focus = p;
    // Open it in the model too, so panels that look at the dock find it.
    app.show_panel(p);
    app.ui.start_screen = false;
}

/// Every other panel, as a list sliding up over the bottom bar.
fn more_sheet(app: &mut EffectcraftApp, ui: &mut egui::Ui, full: Rect, nav: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    let mut close = backdrop(&ctx, full, Color32::from_black_alpha(90));
    const PANELS: [PanelKind; 20] = [
        PanelKind::Project,
        PanelKind::Timeline,
        PanelKind::EffectControls,
        PanelKind::EffectsPresets,
        PanelKind::Layer,
        PanelKind::Properties,
        PanelKind::Info,
        PanelKind::Preview,
        PanelKind::Audio,
        PanelKind::Character,
        PanelKind::Paragraph,
        PanelKind::Align,
        PanelKind::Tracker,
        PanelKind::Paint,
        PanelKind::Brushes,
        PanelKind::RenderQueue,
        PanelKind::History,
        PanelKind::Markers,
        PanelKind::Progress,
        PanelKind::ScriptConsole,
    ];
    let width = (full.width() - 16.0).max(120.0);
    let height = (full.height() * 0.6).min(nav.min.y - full.min.y - 16.0).max(120.0);
    let mut chosen = None;
    egui::Area::new(egui::Id::new("m-more")).order(egui::Order::Foreground).fixed_pos(pos2(full.min.x + 8.0, nav.min.y - height - 6.0)).show(&ctx, |ui| {
        panel_frame(&t).show(ui, |ui| {
            ui.set_width(width - 20.0);
            egui::ScrollArea::vertical().max_height(height - 20.0).show(ui, |ui| {
                for p in PANELS {
                    let title = crate::i18n::tr(p.title());
                    let on = app.mobile.tab == p;
                    if ui.add_sized([ui.available_width(), 42.0], egui::Button::new(title).selected(on)).clicked() {
                        chosen = Some(p);
                    }
                }
            });
        });
    });
    if let Some(p) = chosen {
        select_tab(app, p);
        close = true;
    }
    if close {
        app.mobile.more_open = false;
    }
}

// ------------------------------------------------------------------------ menu drawer

/// All menus of the desktop menu bar in a drawer from the left.
fn menu_drawer(app: &mut EffectcraftApp, ui: &mut egui::Ui, full: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    let mut close = backdrop(&ctx, full, Color32::from_black_alpha(150));
    let width = (full.width() * 0.88).min(380.0);
    let names: Vec<String> =
        effectcraft_engine::menus::menu_bar().iter().filter_map(|n| if let MenuNode::Submenu { label, .. } = n { Some(label.to_string()) } else { None }).collect();
    let mut clicked: Option<(String, serde_json::Value)> = None;
    let mut go_home = false;
    let mut desktop = false;
    egui::Area::new(egui::Id::new("m-drawer")).order(egui::Order::Foreground).fixed_pos(full.min).show(&ctx, |ui| {
        egui::Frame::new().fill(t.panel_bg).stroke(Stroke::new(1.0, t.field_border)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(width - 20.0);
            ui.set_min_height(full.height() - 20.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("EffectCraft").font(Tokens::semibold(18.0)).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_sized([BTN, BTN], egui::Button::new("X")).clicked() {
                        close = true;
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.add_sized([(width - 36.0) / 2.0, 42.0], egui::Button::new(crate::i18n::tr("Home"))).clicked() {
                    go_home = true;
                }
                if ui.add_sized([(width - 36.0) / 2.0, 42.0], egui::Button::new("Desktop layout")).clicked() {
                    desktop = true;
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(full.height() - 130.0).show(ui, |ui| {
                for name in &names {
                    let title = crate::i18n::label(app, "", name);
                    egui::CollapsingHeader::new(egui::RichText::new(title).font(Tokens::ui(16.0))).id_salt(("m-menu", name.as_str())).show(ui, |ui| {
                        if let Some(choice) = menus::menu_contents(app, ui, name) {
                            clicked = Some(choice);
                        }
                    });
                }
            });
        });
    });
    if go_home {
        app.ui.start_screen = true;
        close = true;
    }
    if desktop {
        app.mobile.force = Some(false);
        close = true;
    }
    if let Some((id, params)) = clicked {
        let (id, params) = menus::menu_choice(app, id, params);
        if let Err(e) = menus::invoke(app, &ctx, &id, if params.is_null() { json!({}) } else { params }) {
            app.ui.status = e;
        }
        close = true;
    }
    if close {
        app.mobile.menu_open = false;
    }
}

/// A click-catching sheet behind a popover; true when it was clicked.
fn backdrop(ctx: &egui::Context, full: Rect, fill: Color32) -> bool {
    let mut clicked = false;
    egui::Area::new(egui::Id::new("m-backdrop")).order(egui::Order::Middle).fixed_pos(full.min).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(full.size(), Sense::click());
        ui.painter().rect_filled(rect, 0.0, fill);
        clicked = resp.clicked();
    });
    clicked
}

fn panel_frame(t: &Tokens) -> egui::Frame {
    egui::Frame::new().fill(t.panel_bg).stroke(Stroke::new(1.0, t.field_border)).corner_radius(egui::CornerRadius::same(10)).inner_margin(egui::Margin::same(10))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_shows_the_timeline_with_the_viewer_above() {
        let s = State::default();
        assert_eq!(s.tab, PanelKind::Timeline);
        assert!(s.frac_portrait > 0.2 && s.frac_portrait < 0.8);
        assert!(s.force.is_none());
        assert!(!s.viewer_full && !s.menu_open && !s.more_open);
    }

    #[test]
    fn only_non_viewer_panels_become_the_lower_tab() {
        assert!(is_tab_panel(PanelKind::Timeline));
        assert!(is_tab_panel(PanelKind::EffectControls));
        assert!(!is_tab_panel(PanelKind::Composition));
        assert!(!is_tab_panel(PanelKind::Viewer(2)));
    }

    #[test]
    fn nav_items_are_distinct_panels() {
        let items = nav_items();
        for (i, a) in items.iter().enumerate() {
            for b in items.iter().skip(i + 1) {
                assert_ne!(a.0, b.0);
            }
        }
    }
}
