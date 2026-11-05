#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Bilan Shingrix — saisir les doses, voir le bilan, comparer l'équipe,
//! suivre les rappels, tirer le PDF. Le visage est celui d'Emacs : un
//! tampon, une barre d'onglets, une ligne de mode, une zone d'écho.
//!
//! Un fichier JSON dans le dossier de données (`BILAN_FILE` pour un autre,
//! `BILAN_TAB` pour ouvrir sur un onglet).

mod import;
mod model;
mod pdf;
mod theme;

use eframe::egui::{self, Align, Color32, Key, Layout, RichText, Sense, UiBuilder, Vec2};
use model::{Dose, Due, Patient, Pending, Stats, Store};
use theme as t;

fn main() -> eframe::Result {
    let path = std::env::var_os("BILAN_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(Store::default_path);
    // `BILAN_SIZE=1024x700` : la taille de la fenêtre, pour les captures.
    let size = std::env::var("BILAN_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1200.0, 780.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([980.0, 620.0])
            .with_title("Shingrix"),
        ..Default::default()
    };
    eframe::run_native(
        "bilan-shingrix",
        options,
        Box::new(move |cc| {
            t::apply(&cc.egui_ctx);
            Ok(Box::new(App::new(path)))
        }),
    )
}

/// Une ligne en cours de saisie : du texte, lu à l'enregistrement.
#[derive(Clone, Default)]
struct Buf {
    date: String,
    rank: u8,
    name: String,
    age: String,
    birth: String,
    operator: String,
    prescriber: model::Prescriber,
}

impl Buf {
    /// Une ligne vide qui garde la date et l'opérateur.
    fn cleared(&self) -> Buf {
        Buf {
            date: self.date.clone(),
            operator: self.operator.clone(),
            rank: 1,
            prescriber: model::Prescriber::Officine,
            ..Buf::default()
        }
    }

    fn of(d: &Dose) -> Buf {
        Buf {
            date: model::french(&d.date),
            rank: d.rank,
            name: d.name.clone(),
            age: d.age.map(|a| a.to_string()).unwrap_or_default(),
            birth: model::french(&d.birth),
            operator: d.operator.clone(),
            prescriber: d.prescriber,
        }
    }

    fn read(&self, id: u64, today: &str) -> Result<Dose, String> {
        let operator =
            model::operator_key(&self.operator).ok_or("Opérateur manquant : ses initiales")?;
        let date = match self.date.trim() {
            "" => today.to_owned(),
            s => model::parse_date(s, today).ok_or("Date illisible")?,
        };
        let birth = match self.birth.trim() {
            "" => String::new(),
            s => {
                let b = model::parse_date(s, today).ok_or("DDN illisible")?;
                if b > date {
                    return Err("DDN après la date de la dose".into());
                }
                b
            }
        };
        let age = match self.age.trim() {
            "" => None,
            s => match s.parse::<u8>() {
                Ok(a) if (18..=115).contains(&a) => Some(a),
                _ => return Err("Âge : 18 à 115".into()),
            },
        };
        Ok(Dose {
            id,
            date,
            rank: if self.rank == 2 { 2 } else { 1 },
            name: self.name.split_whitespace().collect::<Vec<_>>().join(" "),
            age,
            prescriber: self.prescriber,
            birth,
            operator,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Period {
    Months12,
    Civil,
    LastCivil,
    All,
}

impl Period {
    const ALL: [Period; 4] = [
        Period::Months12,
        Period::Civil,
        Period::LastCivil,
        Period::All,
    ];
    fn label(self) -> &'static str {
        match self {
            Period::Months12 => "12 mois",
            Period::Civil => "Année",
            Period::LastCivil => "N-1",
            Period::All => "Tout",
        }
    }
}

enum RowAction {
    Edit(Dose),
    Save,
    Cancel,
    AskDelete(u64),
    Delete(u64),
}

struct App {
    path: std::path::PathBuf,
    store: Store,
    locked: bool,
    today: String,
    tab: usize,
    entry: Buf,
    rank_set: bool,
    pick: Option<usize>,
    focus: Option<&'static str>,
    edit: Option<(u64, Buf)>,
    confirm_delete: Option<u64>,
    search: String,
    people: Vec<Patient>,
    births: std::collections::HashMap<String, String>,
    pending: Vec<Pending>,
    expected: Vec<(String, usize)>,
    stats: Stats,
    dirty: bool,
    period: Period,
    due_filter: Option<Due>,
    /// Le message de la zone d'écho.
    note: Option<String>,
    /// Ce que dit l'élément survolé (`help-echo`), le temps d'une image.
    hover: Option<String>,
    /// Un import en cours : le tampon `*Import*` remplace l'onglet.
    import: Option<ImportView>,
}

/// Un fichier ouvert pour l'import, le choix de ses colonnes et ce qu'il
/// donnerait.
struct ImportView {
    file: String,
    table: import::Table,
    mapping: import::Mapping,
    /// Le choix dont `preview` est le résultat.
    seen: Option<import::Mapping>,
    preview: Vec<Dose>,
    result: model::Imported,
}

impl App {
    fn new(path: std::path::PathBuf) -> Self {
        let (store, locked, note) = match Store::load(&path) {
            Ok(s) => (s, false, None),
            Err(e) => (Store::default(), true, Some(format!("{e} — lecture seule"))),
        };
        let today = model::today();
        let mut app = Self {
            path,
            store,
            locked,
            entry: Buf {
                date: model::french(&today),
                rank: 1,
                prescriber: model::Prescriber::Officine,
                ..Buf::default()
            },
            today,
            tab: std::env::var("BILAN_TAB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            rank_set: false,
            pick: None,
            focus: Some("in_name"),
            edit: None,
            confirm_delete: None,
            search: String::new(),
            people: Vec::new(),
            births: Default::default(),
            pending: Vec::new(),
            expected: Vec::new(),
            stats: Stats::default(),
            dirty: true,
            period: Period::Months12,
            due_filter: None,
            note,
            hover: None,
            import: None,
        };
        app.scene();
        app
    }

    /// `BILAN_SCENE` met la fenêtre dans un état à regarder, pour les
    /// captures : `connus` (un début de nom tapé), `modifier` (la ligne
    /// la plus récente ouverte), `supprimer` (la question posée).
    fn scene(&mut self) {
        if let Ok(scene) = std::env::var("BILAN_SCENE") {
            self.set_scene(&scene);
        }
    }

    fn set_scene(&mut self, scene: &str) {
        let last = self
            .store
            .doses
            .iter()
            .max_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)))
            .cloned();
        match scene {
            "connus" => {
                self.entry.name = "mar".into();
                self.entry.operator = "ys".into();
                self.pick = Some(1);
            }
            "import" => {
                self.open_import("exemple-lgpi.csv", import::SAMPLE);
                if let Some(v) = self.import.as_mut() {
                    v.mapping.operator_codes = "03=YS 05=AS".into();
                }
            }
            "modifier" | "supprimer" => {
                if let Some(d) = last {
                    if scene == "supprimer" {
                        self.confirm_delete = Some(d.id);
                    }
                    self.edit = Some((d.id, Buf::of(&d)));
                    self.focus = None;
                }
            }
            _ => {}
        }
    }

    fn bounds(&self) -> (String, String) {
        let y: i32 = self.today[..4].parse().unwrap_or(2026);
        match self.period {
            Period::Months12 => (
                model::add_months(&format!("{}-01", &self.today[..7]), -11),
                self.today.clone(),
            ),
            Period::Civil => (format!("{y}-01-01"), self.today.clone()),
            Period::LastCivil => (format!("{}-01-01", y - 1), format!("{}-12-31", y - 1)),
            Period::All => (String::new(), self.today.clone()),
        }
    }

    fn period_said(&self) -> String {
        let (from, to) = self.bounds();
        let from = if from.is_empty() {
            self.stats
                .months
                .first()
                .map(|m| format!("{}-01", m.0))
                .unwrap_or_else(|| to.clone())
        } else {
            from
        };
        format!("{} – {}", model::french(&from), model::french(&to))
    }

    fn refresh(&mut self) {
        self.people = model::patients(&self.store.doses);
        self.births = self
            .people
            .iter()
            .filter(|p| !p.birth.is_empty())
            .map(|p| (model::fold(&p.name), p.birth.clone()))
            .collect();
        self.pending = model::pending(&self.people, &self.today);
        self.expected = model::expected_by_month(&self.pending, &self.today, 6);
        let (from, to) = self.bounds();
        self.stats = model::stats(&self.store.doses, &from, &to);
        self.dirty = false;
    }

    fn save(&mut self) {
        self.dirty = true;
        if self.locked {
            return;
        }
        if let Err(e) = self.store.save(&self.path) {
            self.note = Some(e.to_string());
        }
    }

    fn submit(&mut self) {
        if self.locked {
            return;
        }
        match self.entry.read(0, &self.today) {
            Ok(d) => {
                let mut said = format!(
                    "D{} enregistrée : {}, {}",
                    d.rank,
                    model::french(&d.date),
                    d.operator
                );
                if !d.name.is_empty() {
                    said.push_str(&format!(", {}", d.name));
                }
                self.store.add(d);
                self.save();
                self.note = Some(said);
                self.entry = self.entry.cleared();
                self.rank_set = false;
                self.pick = None;
                self.focus = Some("in_name");
            }
            Err(e) => {
                if model::operator_key(&self.entry.operator).is_none() {
                    self.focus = Some("in_op");
                }
                self.note = Some(e);
            }
        }
    }

    /// Reprendre un patient connu : son nom, sa DDN, la dose qu'il attend.
    fn take(&mut self, p: &Patient) {
        self.entry.name = p.name.clone();
        if !p.birth.is_empty() {
            self.entry.birth = model::french(&p.birth);
            self.entry.age.clear();
        } else if let Some(a) = p.age {
            self.entry.age = a.to_string();
        }
        self.entry.rank = if !p.d1.is_empty() && p.d2.is_empty() {
            2
        } else {
            1
        };
        self.rank_set = true;
        self.pick = None;
        self.focus = Some("in_date");
    }
}

/// Une cellule peinte : du texte, sa fonte, sa couleur, son alignement.
struct Cell {
    text: String,
    bold: bool,
    color: Color32,
    right: bool,
}

fn cl(text: impl Into<String>) -> Cell {
    Cell {
        text: text.into(),
        bold: false,
        color: t::FG,
        right: false,
    }
}

impl Cell {
    fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    fn color(mut self, c: Color32) -> Self {
        self.color = c;
        self
    }
    fn right(mut self) -> Self {
        self.right = true;
        self
    }
}

/// Une ligne de tableau peinte d'un trait, comme `tabulated-list-mode` :
/// chaque cellule à sa largeur, coupée à son bord, les nombres calés à
/// droite à un caractère du bord.
fn paint_row(ui: &egui::Ui, rect: egui::Rect, widths: &[f32], cells: &[Cell]) {
    let c = t::ch(ui);
    let mut x = rect.left() + c;
    for (w, cell) in widths.iter().zip(cells) {
        let clip = egui::Rect::from_min_max(
            egui::pos2(x, rect.top()),
            egui::pos2(x + w - c * 0.5, rect.bottom()),
        );
        let p = ui.painter_at(clip.intersect(ui.clip_rect()));
        let font = if cell.bold { t::bold_font() } else { t::mono() };
        if cell.right {
            p.text(
                egui::pos2(x + w - 2.0 * c, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                &cell.text,
                font,
                cell.color,
            );
        } else {
            p.text(
                egui::pos2(x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &cell.text,
                font,
                cell.color,
            );
        }
        x += w;
    }
}

/// Une cellule qui prend exactement sa largeur, pour des widgets.
fn cell<R>(ui: &mut egui::Ui, w: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let h = t::row_h(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    add(&mut child)
}

fn read_enter(ui: &egui::Ui, r: &egui::Response) -> bool {
    r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
}

/// La couleur d'un opérateur, par son rang.
fn op_color(i: usize) -> Color32 {
    t::SERIES[i % t::SERIES.len()]
}

/// Les colonnes de la saisie, en caractères : date, dose, patient (le
/// reste), âge, DDN, opérateur, prescripteur, boutons.
fn entry_columns(ui: &egui::Ui) -> [f32; 8] {
    let c = t::ch(ui);
    let fixed = [13.0, 15.0, 0.0, 6.0, 13.0, 7.0, 19.0, 22.0].map(|n: f32| n * c);
    let name = (ui.available_width() - fixed.iter().sum::<f32>()).max(20.0 * c);
    [
        fixed[0], fixed[1], name, fixed[3], fixed[4], fixed[5], fixed[6], fixed[7],
    ]
}

/// Les colonnes du registre : date, dose, patient, âge, DDN, opérateur,
/// prescripteur, boutons de la ligne en cours de modification.
fn register_columns(ui: &egui::Ui) -> [f32; 8] {
    let c = t::ch(ui);
    let fixed = [13.0, 6.0, 0.0, 6.0, 13.0, 7.0, 8.0, 34.0].map(|n: f32| n * c);
    let name = (ui.available_width() - fixed.iter().sum::<f32>() - 2.0 * c).max(20.0 * c);
    [
        fixed[0], fixed[1], name, fixed[3], fixed[4], fixed[5], fixed[6], fixed[7],
    ]
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}

impl App {
    fn frame(&mut self, ctx: &egui::Context) {
        if self.dirty {
            self.refresh();
        }
        // La question de la zone d'écho prend les touches : o, n, Échap.
        if let Some(id) = self.confirm_delete {
            let (yes, no) = ctx.input_mut(|i| {
                let yes = i.consume_key(egui::Modifiers::NONE, Key::O)
                    || i.consume_key(egui::Modifiers::NONE, Key::Y);
                let no = i.consume_key(egui::Modifiers::NONE, Key::N)
                    || i.consume_key(egui::Modifiers::NONE, Key::Escape);
                i.events.retain(|e| !matches!(e, egui::Event::Text(_)));
                (yes, no)
            });
            if yes {
                self.apply(RowAction::Delete(id));
            } else if no {
                self.confirm_delete = None;
                self.note = Some("Abandon".into());
            }
        } else if ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Text(_) | egui::Event::Key { pressed: true, .. }
                )
            })
        }) {
            // Comme Emacs : la frappe suivante efface le message.
            self.note = None;
        }
        if self.tab == 0 && self.edit.is_none() {
            let (one, two) = ctx.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::COMMAND, Key::Num1),
                    i.consume_key(egui::Modifiers::COMMAND, Key::Num2),
                )
            });
            if one || two {
                self.entry.rank = if two { 2 } else { 1 };
                self.rank_set = true;
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::P)) {
            self.make_pdf();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Tab)) {
            self.tab = (self.tab + 1) % TABS.len();
        }
        if self.tab != 0 {
            self.confirm_delete = None;
        }
        let hover = self.hover.take();

        egui::TopBottomPanel::top("menu")
            .frame(
                egui::Frame::none()
                    .fill(t::GREY90)
                    .inner_margin(egui::Margin::symmetric(4.0, 1.0)),
            )
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = t::ch(ui);
                ui.spacing_mut().button_padding = Vec2::new(t::ch(ui) * 0.5, 2.0);
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("Fichier", |ui| {
                        ui.set_min_width(28.0 * t::ch(ui));
                        if ui
                            .add(egui::Button::new("Bilan PDF…").shortcut_text("C-p"))
                            .clicked()
                        {
                            ui.close_menu();
                            self.make_pdf();
                        }
                        if ui
                            .add_enabled(!self.locked, egui::Button::new("Importer CSV…"))
                            .clicked()
                        {
                            ui.close_menu();
                            self.import_csv();
                        }
                        if ui.button("Exporter CSV…").clicked() {
                            ui.close_menu();
                            self.export_csv();
                        }
                        if ui.button("Modèle du bilan…").clicked() {
                            ui.close_menu();
                            self.edit_template();
                        }
                        if ui
                            .add_enabled(
                                self.template_path().exists(),
                                egui::Button::new("Modèle d'origine"),
                            )
                            .clicked()
                        {
                            ui.close_menu();
                            self.reset_template();
                        }
                        if ui
                            .add(egui::Button::new("Quitter").shortcut_text("C-q"))
                            .clicked()
                        {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("Onglets", |ui| {
                        ui.set_min_width(28.0 * t::ch(ui));
                        for (i, l) in TABS.iter().enumerate() {
                            if ui.button(*l).clicked() {
                                ui.close_menu();
                                self.tab = i;
                            }
                        }
                        if ui
                            .add(egui::Button::new("Onglet suivant").shortcut_text("C-TAB"))
                            .clicked()
                        {
                            ui.close_menu();
                            self.tab = (self.tab + 1) % TABS.len();
                        }
                    });
                });
            });

        egui::TopBottomPanel::top("tabs")
            .frame(egui::Frame::none().fill(t::GREY85))
            .show_separator_line(false)
            .show(ctx, |ui| {
                t::tab_bar(ui, &mut self.tab, &TABS);
            });

        egui::TopBottomPanel::bottom("echo")
            .frame(
                egui::Frame::none()
                    .fill(t::BG)
                    .inner_margin(egui::Margin::symmetric(2.0, 1.0)),
            )
            .show_separator_line(false)
            .show(ctx, |ui| {
                let h = t::row_h(ui);
                let (r, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
                let at = egui::pos2(r.left() + 2.0, r.center().y);
                let p = ui.painter();
                if let Some(prompt) = self.delete_prompt() {
                    p.text(at, egui::Align2::LEFT_CENTER, prompt, t::mono(), t::PROMPT);
                } else if let Some(h) = &hover {
                    p.text(at, egui::Align2::LEFT_CENTER, h, t::mono(), t::FG);
                } else if let Some(n) = &self.note {
                    p.text(at, egui::Align2::LEFT_CENTER, n, t::mono(), t::FG);
                }
            });

        egui::TopBottomPanel::bottom("mode")
            .frame(egui::Frame::none().fill(t::MODE_LINE))
            .show_separator_line(false)
            .show(ctx, |ui| {
                let flag = if self.locked { "-:%%-  " } else { "-:---  " };
                let name = if self.import.is_some() {
                    format!("{:<12}", "*Import*")
                } else {
                    format!("{:<12}", TABS[self.tab.min(TABS.len() - 1)])
                };
                let info = self.mode_info();
                t::mode_line(
                    ui,
                    &[(flag, false), (&name, true), ("  ", false), (&info, false)],
                    "(Shingrix)",
                );
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(t::BG)
                    .inner_margin(egui::Margin::symmetric(8.0, 6.0)),
            )
            .show(ctx, |ui| match self.tab {
                _ if self.import.is_some() => self.import_view(ui),
                0 => self.entry_tab(ui),
                1 => self.bilan_tab(ui),
                2 => self.team_tab(ui),
                _ => self.recall_tab(ui),
            });

        if self.hover != hover {
            ctx.request_repaint();
        }
    }
}

const TABS: [&str; 4] = ["Saisie", "Bilan", "Équipe", "Rappels"];

impl App {
    /// Ce que dit la ligne de mode, après le nom du tampon.
    fn mode_info(&self) -> String {
        let due = self.pending.iter().filter(|p| p.due != Due::Soon).count();
        let mut out = format!(
            "{} doses  {} patients  {} rappels",
            self.store.doses.len(),
            self.people.len(),
            due
        );
        if matches!(self.tab, 1 | 2) {
            out.push_str(&format!("   {}", self.period_said()));
        }
        if self.locked {
            out.push_str("   lecture seule");
        }
        out
    }

    fn delete_prompt(&self) -> Option<String> {
        let id = self.confirm_delete?;
        let d = self.store.doses.iter().find(|d| d.id == id)?;
        let who = if d.name.is_empty() {
            String::new()
        } else {
            format!(", {}", d.name)
        };
        Some(format!(
            "Supprimer la D{} du {}{} ? (o ou n) ",
            d.rank,
            model::french(&d.date),
            who
        ))
    }
}

// ---------------------------------------------------------------------
// Saisie
// ---------------------------------------------------------------------

impl App {
    fn entry_tab(&mut self, ui: &mut egui::Ui) {
        let c = t::ch(ui);
        let rh = t::row_h(ui);
        t::heading(ui, "Nouvelle dose", "Entrée enregistre, C-1 / C-2 la dose");
        let cols = entry_columns(ui);
        ui.horizontal(|ui| {
            for (w, n) in cols.iter().zip([
                "Date", "Dose", "Patient", "Âge", "DDN", "Opér.", "Prescrit", "",
            ]) {
                cell(ui, *w, |ui| ui.label(t::shadow(n)));
            }
        });
        if let Some(id) = self.focus.take() {
            ui.memory_mut(|m| m.request_focus(egui::Id::new(id)));
        }
        let suggestions: Vec<Patient> = model::suggest(&self.people, &self.entry.name, 6)
            .into_iter()
            .filter(|p| {
                model::fold(&p.name) != model::fold(&self.entry.name) || self.pick.is_some()
            })
            .cloned()
            .collect();
        let (mut enter, mut name_enter, mut name_changed, mut add) = (false, false, false, false);
        ui.horizontal(|ui| {
            cell(ui, cols[0], |ui| {
                let r = t::field(
                    ui,
                    "in_date",
                    &mut self.entry.date,
                    cols[0] - c,
                    "jj/mm/aaaa",
                );
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[1], |ui| {
                if t::radio(ui, "D1", self.entry.rank != 2).clicked() {
                    self.entry.rank = 1;
                    self.rank_set = true;
                }
                if t::radio(ui, "D2", self.entry.rank == 2).clicked() {
                    self.entry.rank = 2;
                    self.rank_set = true;
                }
            });
            cell(ui, cols[2], |ui| {
                let r = t::field(
                    ui,
                    "in_name",
                    &mut self.entry.name,
                    cols[2] - c,
                    "facultatif",
                );
                if r.has_focus() && !suggestions.is_empty() {
                    let (down, up) = ui.input_mut(|i| {
                        (
                            i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                            i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                        )
                    });
                    let n = suggestions.len();
                    if down {
                        self.pick = Some(self.pick.map_or(0, |p| (p + 1) % n));
                    }
                    if up {
                        self.pick = Some(self.pick.map_or(n - 1, |p| (p + n - 1) % n));
                    }
                }
                name_changed = r.changed();
                name_enter = read_enter(ui, &r);
            });
            cell(ui, cols[3], |ui| {
                let r = t::field(ui, "in_age", &mut self.entry.age, cols[3] - c, "");
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[4], |ui| {
                let r = t::field(ui, "in_birth", &mut self.entry.birth, cols[4] - c, "");
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[5], |ui| {
                let r = t::field(ui, "in_op", &mut self.entry.operator, cols[5] - c, "");
                let shown = self.entry.operator.to_uppercase();
                if shown != self.entry.operator {
                    self.entry.operator = shown;
                }
                if model::operator_key(&self.entry.operator).is_none() {
                    t::squiggle(ui, r.rect.expand2(Vec2::new(3.0, 3.0)));
                }
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[6], |ui| {
                use model::Prescriber::{Medecin, Officine};
                if t::radio(ui, "off.", self.entry.prescriber != Medecin).clicked() {
                    self.entry.prescriber = Officine;
                }
                if t::radio(ui, "méd.", self.entry.prescriber == Medecin).clicked() {
                    self.entry.prescriber = Medecin;
                }
            });
            cell(ui, cols[7], |ui| {
                add = ui
                    .add_enabled_ui(!self.locked, |ui| t::button(ui, "Ajouter"))
                    .inner
                    .clicked();
                ui.add_space(c);
                if t::button(ui, "Effacer").clicked() {
                    self.entry = self.entry.cleared();
                    self.rank_set = false;
                    self.focus = Some("in_name");
                }
            });
        });
        if name_changed {
            self.pick = None;
            if !self.rank_set {
                let key = model::fold(&self.entry.name);
                self.entry.rank = match self.people.iter().find(|p| model::fold(&p.name) == key) {
                    Some(p) if !p.d1.is_empty() && p.d2.is_empty() => 2,
                    _ => 1,
                };
            }
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) && self.edit.is_none() {
            self.entry = self.entry.cleared();
            self.rank_set = false;
            self.focus = Some("in_name");
        }
        // Les patients connus qui commencent comme ce qu'on tape : des
        // liens, ↓ ↑ pour choisir, Entrée pour prendre.
        ui.horizontal(|ui| {
            ui.set_min_height(rh);
            ui.add_space(cols[0] + cols[1]);
            if suggestions.is_empty() {
                return;
            }
            ui.label(t::shadow("Connus : "));
            let right_edge = ui.max_rect().right() - c;
            for (i, p) in suggestions.iter().enumerate() {
                let mut said = p.name.clone();
                let mut extra = Vec::new();
                if !p.birth.is_empty() {
                    extra.push(model::french(&p.birth));
                }
                match (p.d1.is_empty(), p.d2.is_empty()) {
                    (false, true) => extra.push("attend D2".into()),
                    (_, false) => extra.push("complet".into()),
                    _ => {}
                }
                if !extra.is_empty() {
                    said.push_str(&format!(" ({})", extra.join(", ")));
                }
                // Ce qui ne tient pas sur la ligne n'est pas montré.
                let w = said.chars().count() as f32 * c;
                if ui.cursor().left() + w > right_edge {
                    ui.label(t::shadow("…"));
                    break;
                }
                if t::link(ui, &said, self.pick == Some(i)).clicked() {
                    self.take(p);
                }
                ui.add_space(2.0 * c);
            }
        });
        if name_enter {
            match self.pick.and_then(|i| suggestions.get(i)) {
                Some(p) => {
                    let p = p.clone();
                    self.take(&p);
                }
                None => enter = true,
            }
        }
        if enter || add {
            self.submit();
        }

        ui.add_space(rh * 0.5);
        let total = self.store.doses.len();
        ui.horizontal(|ui| {
            t::heading(ui, "Registre", &format!("{total} doses"));
            ui.add_space(4.0 * c);
            ui.label(t::shadow("Chercher : "));
            t::field(ui, "search", &mut self.search, 24.0 * c, "nom, date, AS");
            if !self.search.is_empty() {
                ui.add_space(c);
                if t::button(ui, "Tout").clicked() {
                    self.search.clear();
                }
            }
        });
        let cols = register_columns(ui);
        t::header_line(
            ui,
            &cols,
            &[
                "Date", "Dose", "Patient", "Âge", "DDN", "Opér.", "Prescr.", "",
            ],
            c,
        );
        let q = model::fold(&self.search);
        let mut rows: Vec<Dose> = self
            .store
            .doses
            .iter()
            .filter(|d| {
                q.is_empty()
                    || model::fold(&d.name).contains(&q)
                    || model::french(&d.date).contains(self.search.trim())
                    || model::fold(&d.operator) == q
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
        if rows.is_empty() {
            ui.label(t::shadow(if q.is_empty() {
                " Aucune dose."
            } else {
                " Rien ne correspond."
            }));
            return;
        }
        let mut action: Option<RowAction> = None;
        egui::ScrollArea::vertical()
            .id_salt("registre")
            .auto_shrink([false, false])
            .show_rows(ui, rh, rows.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for i in range {
                    if let Some(a) = self.row(ui, &rows[i], &cols) {
                        action = Some(a);
                    }
                }
            });
        if let Some(a) = action {
            self.apply(a);
        }
    }

    fn row(&mut self, ui: &mut egui::Ui, d: &Dose, cols: &[f32; 8]) -> Option<RowAction> {
        let c = t::ch(ui);
        let rh = t::row_h(ui);
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::click());
        let editing = self.edit.as_ref().is_some_and(|(id, _)| *id == d.id);
        if editing {
            ui.painter().rect_filled(rect, 0.0, t::REGION);
        } else if resp.hovered() && !self.locked {
            ui.painter().rect_filled(rect, 0.0, t::HIGHLIGHT);
        }
        let mut out = None;
        if editing {
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(rect.shrink2(Vec2::new(c, 0.0)))
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let ui = &mut child;
            let (_, buf) = self.edit.as_mut().expect("editing");
            let mut enter = false;
            cell(ui, cols[0], |ui| {
                let r = t::field(ui, "ed_date", &mut buf.date, cols[0] - c, "");
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[1], |ui| {
                let said = if buf.rank == 2 { "D2" } else { "D1" };
                if t::button(ui, said).clicked() {
                    buf.rank = if buf.rank == 2 { 1 } else { 2 };
                }
            });
            cell(ui, cols[2], |ui| {
                let r = t::field(ui, "ed_name", &mut buf.name, cols[2] - c, "");
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[3], |ui| {
                let r = t::field(ui, "ed_age", &mut buf.age, cols[3] - c, "");
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[4], |ui| {
                let r = t::field(ui, "ed_birth", &mut buf.birth, cols[4] - c, "");
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[5], |ui| {
                let r = t::field(ui, "ed_op", &mut buf.operator, cols[5] - c, "");
                let shown = buf.operator.to_uppercase();
                if shown != buf.operator {
                    buf.operator = shown;
                }
                if model::operator_key(&buf.operator).is_none() {
                    t::squiggle(ui, r.rect.expand2(Vec2::new(3.0, 3.0)));
                }
                enter |= read_enter(ui, &r);
            });
            cell(ui, cols[6], |ui| {
                if t::button(ui, buf.prescriber.short()).clicked() {
                    buf.prescriber = buf.prescriber.next();
                }
            });
            cell(ui, cols[7], |ui| {
                if t::button(ui, "Enregistrer").clicked() || enter {
                    out = Some(RowAction::Save);
                }
                ui.add_space(c);
                if t::button(ui, "Annuler").clicked()
                    || (self.confirm_delete.is_none() && ui.input(|i| i.key_pressed(Key::Escape)))
                {
                    out = Some(RowAction::Cancel);
                }
                ui.add_space(c);
                if t::button(ui, "Supprimer").clicked() {
                    out = Some(if self.confirm_delete == Some(d.id) {
                        RowAction::Delete(d.id)
                    } else {
                        RowAction::AskDelete(d.id)
                    });
                }
            });
            return out;
        }
        // La DDN d'une autre dose du même patient, en `shadow`.
        let inherited = (d.birth.is_empty() && !d.name.is_empty())
            .then(|| self.births.get(&model::fold(&d.name)))
            .flatten();
        let age = match inherited {
            Some(b) => model::age_on(b, &d.date),
            None => model::dose_age(d),
        };
        let none = || cl("-").color(t::SHADOW);
        let cells = [
            cl(model::french(&d.date)),
            cl(if d.rank == 2 { "D2" } else { "D1" }),
            if d.name.is_empty() {
                none()
            } else {
                cl(d.name.clone())
            },
            age.map_or_else(none, |a| cl(a.to_string())),
            match (d.birth.is_empty(), inherited) {
                (false, _) => cl(model::french(&d.birth)),
                (true, Some(b)) => cl(model::french(b)).color(t::SHADOW),
                (true, None) => none(),
            },
            model::operator_key(&d.operator).map_or_else(none, |k| cl(k).bold()),
            match d.prescriber {
                model::Prescriber::Inconnu => none(),
                model::Prescriber::Medecin => cl("méd.").color(t::SHADOW),
                model::Prescriber::Officine => cl("off."),
            },
            if resp.hovered() && !self.locked {
                cl("clic : modifier").color(t::GREY20)
            } else {
                cl("")
            },
        ];
        paint_row(ui, rect, cols, &cells);
        if resp.clicked() && !self.locked {
            out = Some(RowAction::Edit(d.clone()));
        }
        out
    }

    fn apply(&mut self, a: RowAction) {
        match a {
            RowAction::Edit(d) => {
                self.edit = Some((d.id, Buf::of(&d)));
                self.confirm_delete = None;
                self.focus = None;
            }
            RowAction::Cancel => {
                self.edit = None;
                self.confirm_delete = None;
            }
            RowAction::Save => {
                if let Some((id, buf)) = &self.edit {
                    match buf.read(*id, &self.today) {
                        Ok(d) => {
                            self.store.replace(d);
                            self.edit = None;
                            self.save();
                            self.note = Some("Dose modifiée".into());
                        }
                        Err(e) => self.note = Some(e),
                    }
                }
            }
            RowAction::AskDelete(id) => self.confirm_delete = Some(id),
            RowAction::Delete(id) => {
                if let Some(d) = self.store.remove(id) {
                    self.note = Some(format!(
                        "D{} du {} supprimée",
                        d.rank,
                        model::french(&d.date)
                    ));
                }
                self.edit = None;
                self.confirm_delete = None;
                self.save();
            }
        }
    }
}

// ---------------------------------------------------------------------
// Bilan, équipe, rappels
// ---------------------------------------------------------------------

impl App {
    fn period_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(t::shadow("Période : "));
            for p in Period::ALL {
                if t::radio(ui, p.label(), self.period == p).clicked() && self.period != p {
                    self.period = p;
                    self.dirty = true;
                }
                ui.add_space(t::ch(ui));
            }
            ui.add_space(2.0 * t::ch(ui));
            ui.label(t::shadow(self.period_said()));
        });
    }

    /// Les chiffres, alignés en colonnes comme une sortie de `describe` :
    /// libellé, valeur en gras calée à droite, précision en `shadow`.
    fn figures(ui: &mut egui::Ui, items: &[[String; 4]], per_col: usize) {
        let c = t::ch(ui);
        let rh = t::row_h(ui);
        let cols = items.len().div_ceil(per_col.max(1));
        let block = [11.0 * c, 9.0 * c, 18.0 * c];
        let width = (block.iter().sum::<f32>() + 6.0 * c)
            .min((ui.available_width() - c) / cols.max(1) as f32);
        for r in 0..per_col.min(items.len()) {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(width * cols as f32 + c, rh), Sense::hover());
            let p = ui.painter();
            for k in 0..cols {
                let Some([v, l, sub, color]) = items.get(k * per_col + r) else {
                    continue;
                };
                let x = rect.left() + c + k as f32 * width;
                let y = rect.center().y;
                p.text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    l,
                    t::mono(),
                    t::FG,
                );
                let alert = color == "red" && v != "0";
                p.text(
                    egui::pos2(x + block[0] + block[1] - c, y),
                    egui::Align2::RIGHT_CENTER,
                    v,
                    t::bold_font(),
                    if alert { t::ERROR } else { t::FG },
                );
                p.text(
                    egui::pos2(x + block[0] + block[1] + c, y),
                    egui::Align2::LEFT_CENTER,
                    sub,
                    t::mono(),
                    t::SHADOW,
                );
            }
        }
    }

    /// Deux fenêtres côte à côte, séparées par la bordure verticale.
    fn split(
        &mut self,
        ui: &mut egui::Ui,
        left_share: f32,
        height: f32,
        left: impl FnOnce(&mut Self, &mut egui::Ui),
        right: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        let c = t::ch(ui);
        let w = ui.available_width();
        let lw = (w * left_share).round();
        let top = ui.cursor().top();
        let mut bottom = top;
        ui.horizontal_top(|ui| {
            let r = ui
                .allocate_ui_with_layout(
                    Vec2::new(lw - c, height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(lw - c);
                        ui.set_min_height(height);
                        left(self, ui);
                    },
                )
                .response
                .rect;
            bottom = bottom.max(r.bottom());
            ui.add_space(2.0 * c);
            let r = ui
                .allocate_ui_with_layout(
                    Vec2::new(w - lw - c, height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(w - lw - c);
                        ui.set_min_height(height);
                        right(self, ui);
                    },
                )
                .response
                .rect;
            bottom = bottom.max(r.bottom());
        });
        let x = ui.min_rect().left() + lw + 0.5;
        ui.painter().line_segment(
            [egui::pos2(x, top + 4.0), egui::pos2(x, bottom)],
            egui::Stroke::new(1.0_f32, t::MODE_LINE),
        );
    }

    fn bilan_tab(&mut self, ui: &mut egui::Ui) {
        self.period_bar(ui);
        egui::ScrollArea::vertical()
            .id_salt("bilan")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let tiles = pdf::tiles(&self.stats, &self.pending, &self.expected);
                t::heading(ui, "Synthèse", "");
                Self::figures(ui, &tiles, 4);
                let s = self.stats.clone();
                t::heading(ui, "Doses par mois", "");
                let groups: Vec<(String, Vec<(f64, Color32)>)> = s
                    .months
                    .iter()
                    .map(|(m, a, b)| {
                        (
                            model::month_label(m),
                            vec![(*a as f64, t::D1), (*b as f64, t::D2)],
                        )
                    })
                    .collect();
                self.bars(ui, &groups, 13.0, &["D1", "D2"]);
                legend(ui, &[("D1", t::D1), ("D2", t::D2)]);
                self.split(
                    ui,
                    0.5,
                    0.0,
                    |_, ui| {
                        t::heading(ui, "Âges", "");
                        let rows: Vec<(String, usize, Color32)> = model::BANDS
                            .iter()
                            .map(|b| b.2.to_owned())
                            .chain(["?".to_owned()])
                            .zip(s.ages)
                            .map(|(l, n)| (l, n, t::D1))
                            .collect();
                        text_bars(ui, &rows, s.total());
                    },
                    |app, ui| {
                        t::heading(ui, "D2 à venir", "6 mois");
                        let groups: Vec<(String, Vec<(f64, Color32)>)> = app
                            .expected
                            .iter()
                            .map(|(m, n)| (model::month_label(m), vec![(*n as f64, t::D2)]))
                            .collect();
                        app.bars(ui, &groups, 9.0, &["D2"]);
                    },
                );
            });
    }

    fn team_tab(&mut self, ui: &mut egui::Ui) {
        self.period_bar(ui);
        let s = self.stats.clone();
        egui::ScrollArea::vertical()
            .id_salt("equipe")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                t::heading(ui, "Comparaison", "");
                if s.team.is_empty() {
                    ui.label(t::shadow(" Aucune dose sur la période."));
                    return;
                }
                let c = t::ch(ui);
                let names = [
                    "Opérateur",
                    "Doses",
                    "Part",
                    "D1",
                    "D2",
                    "Off.",
                    "Patients",
                    "Âge moy.",
                    "Délai",
                    "Fenêtre",
                    "Dernière",
                ];
                let widths =
                    [12.0, 8.0, 8.0, 7.0, 7.0, 7.0, 10.0, 11.0, 11.0, 10.0, 12.0].map(|n| n * c);
                t::header_line(ui, &widths, &names, c);
                let total = s.total();
                let rh = t::row_h(ui);
                for (i, o) in s.team.iter().enumerate() {
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::hover());
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, 0.0, t::HIGHLIGHT);
                    }
                    let mut cells = vec![cl(format!("■ {}", o.key)).bold().color(op_color(i))];
                    for (k, v) in pdf::team_cells(o, total).into_iter().enumerate() {
                        cells.push(if k == 9 { cl(v) } else { cl(v).right() });
                    }
                    paint_row(ui, rect, &widths, &cells);
                    // Les initiales en noir, le carré seul en couleur.
                    ui.painter().text(
                        egui::pos2(rect.left() + 3.0 * c, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &o.key,
                        t::bold_font(),
                        t::FG,
                    );
                }
                ui.label(t::shadow(
                    " Off. : prescrites par lui-même.  Délai : médiane D1 → D2.  Fenêtre : D2 entre 2 et 6 mois.",
                ));
                t::heading(ui, "Doses par mois et opérateur", "");
                let groups: Vec<(String, Vec<(f64, Color32)>)> = s
                    .months
                    .iter()
                    .enumerate()
                    .map(|(m, (key, _, _))| {
                        (
                            model::month_label(key),
                            s.team
                                .iter()
                                .enumerate()
                                .map(|(i, o)| (o.months[m] as f64, op_color(i)))
                                .collect(),
                        )
                    })
                    .collect();
                let keys: Vec<&str> = s.team.iter().map(|o| o.key.as_str()).collect();
                self.bars(ui, &groups, 13.0, &keys);
                let legend_items: Vec<(&str, Color32)> = s
                    .team
                    .iter()
                    .enumerate()
                    .map(|(i, o)| (o.key.as_str(), op_color(i)))
                    .collect();
                legend(ui, &legend_items);
                self.split(
                    ui,
                    0.5,
                    0.0,
                    |_, ui| {
                        t::heading(ui, "Part", "");
                        let rows: Vec<(String, usize, Color32)> = s
                            .team
                            .iter()
                            .enumerate()
                            .map(|(i, o)| (o.key.clone(), o.doses, op_color(i)))
                            .collect();
                        text_bars(ui, &rows, s.total());
                    },
                    |app, ui| {
                        t::heading(ui, "D1 et D2", "");
                        let groups: Vec<(String, Vec<(f64, Color32)>)> = s
                            .team
                            .iter()
                            .map(|o| {
                                (
                                    o.key.clone(),
                                    vec![(o.d1 as f64, t::D1), (o.d2 as f64, t::D2)],
                                )
                            })
                            .collect();
                        app.bars(ui, &groups, 9.0, &["D1", "D2"]);
                        legend(ui, &[("D1", t::D1), ("D2", t::D2)]);
                    },
                );
            });
    }

    fn recall_tab(&mut self, ui: &mut egui::Ui) {
        let count = |d: Due| self.pending.iter().filter(|p| p.due == d).count();
        let (late, now, soon) = (count(Due::Late), count(Due::Now), count(Due::Soon));
        let figures = [
            [
                now.to_string(),
                "À faire".into(),
                "2 à 6 mois".into(),
                String::new(),
            ],
            [
                late.to_string(),
                "En retard".into(),
                "plus de 6 mois".into(),
                "red".into(),
            ],
            [
                soon.to_string(),
                "Bientôt".into(),
                "moins de 2 mois".into(),
                String::new(),
            ],
        ];
        t::heading(ui, "Deuxièmes doses", "depuis la D1");
        Self::figures(ui, &figures, 1);
        let rest = ui.available_height();
        let share = 1.0 - (40.0 * t::ch(ui) / ui.available_width()).clamp(0.25, 0.4);
        self.split(
            ui,
            share,
            rest,
            |app, ui| app.recall_list(ui),
            |app, ui| {
                t::heading(ui, "À venir par mois", "");
                let groups: Vec<(String, Vec<(f64, Color32)>)> = app
                    .expected
                    .iter()
                    .map(|(m, n)| (model::month_label(m), vec![(*n as f64, t::D2)]))
                    .collect();
                app.bars(ui, &groups, 12.0, &["D2"]);
            },
        );
    }

    fn recall_list(&mut self, ui: &mut egui::Ui) {
        let c = t::ch(ui);
        let rh = t::row_h(ui);
        ui.horizontal(|ui| {
            t::heading(ui, "Liste", "");
            ui.add_space(2.0 * c);
            for (f, l) in [
                (None, "Tous"),
                (Some(Due::Now), "À faire"),
                (Some(Due::Late), "Retard"),
                (Some(Due::Soon), "Bientôt"),
            ] {
                if t::radio(ui, l, self.due_filter == f).clicked() {
                    self.due_filter = f;
                }
                ui.add_space(c);
            }
            ui.add_space(c);
            if t::button(ui, "Copier").clicked() {
                let text: String = self
                    .pending
                    .iter()
                    .filter(|p| self.due_filter.is_none_or(|f| p.due == f))
                    .map(|p| {
                        format!(
                            "{}\t{}\t{}\t{} – {}\n",
                            p.name,
                            p.due.label(),
                            model::french(&p.d1),
                            model::french(&p.from),
                            model::french(&p.to)
                        )
                    })
                    .collect();
                ui.ctx().copy_text(text);
                self.note = Some("Liste copiée dans le presse-papiers".into());
            }
        });
        let list: Vec<Pending> = self
            .pending
            .iter()
            .filter(|p| self.due_filter.is_none_or(|f| p.due == f))
            .cloned()
            .collect();
        // La fenêtre se lit depuis la D1 : la date de la D1 n'a pas de
        // colonne à elle.
        let fixed = [0.0, 6.0, 22.0, 10.0, 18.0].map(|n| n * c);
        let name_w = (ui.available_width() - fixed.iter().sum::<f32>() - 3.0 * c).max(14.0 * c);
        let widths = [name_w, fixed[1], fixed[2], fixed[3], fixed[4]];
        t::header_line(
            ui,
            &widths,
            &["Patient", "Âge", "Fenêtre D2", "État", "Échéance"],
            c,
        );
        if list.is_empty() {
            ui.label(t::shadow(" Aucun rappel."));
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("rappels")
            .auto_shrink([false, false])
            .show_rows(ui, rh, list.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for p in &list[range] {
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::hover());
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, 0.0, t::HIGHLIGHT);
                    }
                    let short = |d: &str| format!("{}/{}", &model::french(d)[..5], &d[2..4]);
                    let state = match p.due {
                        Due::Late => cl(p.due.label()).color(t::ERROR),
                        Due::Now => cl(p.due.label()).bold(),
                        Due::Soon => cl(p.due.label()).color(t::SHADOW),
                    };
                    let days = match p.due {
                        Due::Soon => format!("ouvre dans {} j", p.days),
                        Due::Now => format!("encore {} j", p.days),
                        Due::Late => format!("passée de {} j", -p.days),
                    };
                    let cells = [
                        cl(if p.name.is_empty() { "-" } else { &p.name }),
                        p.age
                            .map_or_else(|| cl("-").color(t::SHADOW), |a| cl(a.to_string())),
                        cl(format!("{} – {}", short(&p.from), short(&p.to))),
                        state,
                        cl(days).color(t::SHADOW),
                    ];
                    paint_row(ui, rect, &widths, &cells);
                }
            });
    }

    /// Le modèle à soi, à côté du registre.
    fn template_path(&self) -> std::path::PathBuf {
        self.path.with_file_name("bilan.typ")
    }

    /// Copie le modèle d'origine s'il n'y en a pas, et l'ouvre.
    fn edit_template(&mut self) {
        let to = self.template_path();
        if !to.exists() {
            let made = to
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&to, pdf::TEMPLATE));
            if let Err(e) = made {
                self.note = Some(format!("{}: {e}", to.display()));
                return;
            }
        }
        if open::that_detached(&to).is_err() {
            let _ = to.parent().map(open::that_detached);
        }
        self.note = Some(format!("Modèle {} ; C-p le suit", to.display()));
    }

    /// Revient au modèle d'origine ; le modèle à soi est gardé à côté.
    fn reset_template(&mut self) {
        let from = self.template_path();
        let kept = (1..)
            .map(|i| from.with_file_name(format!("bilan-{}-{i}.typ", self.today)))
            .find(|p| !p.exists())
            .unwrap_or_else(|| from.with_extension("typ.ancien"));
        self.note = Some(match std::fs::rename(&from, &kept) {
            Ok(()) => format!("Modèle d'origine ; l'ancien gardé : {}", kept.display()),
            Err(e) => format!("{}: {e}", from.display()),
        });
    }

    fn make_pdf(&mut self) {
        let head = pdf::Head {
            period: self.period_said(),
            edited: model::french(&self.today),
        };
        let own = self.template_path();
        let template = match std::fs::read_to_string(&own) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                self.note = Some(format!("{}: {e}", own.display()));
                return;
            }
        };
        let tpl = template.as_deref().unwrap_or(pdf::TEMPLATE);
        let bytes = match pdf::render(&self.stats, &self.pending, &self.expected, &head, tpl) {
            Ok(b) => b,
            Err(e) => {
                self.note = Some(match template {
                    Some(_) => format!("bilan.typ, {e}"),
                    None => format!("PDF : {e}"),
                });
                return;
            }
        };
        let Some(to) = rfd::FileDialog::new()
            .set_file_name(format!("shingrix-{}.pdf", self.today))
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            self.note = Some("Abandon".into());
            return;
        };
        self.note = Some(match std::fs::write(&to, bytes) {
            Ok(()) => {
                let _ = open::that_detached(&to);
                format!("Écrit {}", to.display())
            }
            Err(e) => format!("{}: {e}", to.display()),
        });
    }

    /// Le choix des colonnes gardé pour le fichier suivant.
    fn mapping_path(&self) -> std::path::PathBuf {
        self.path.with_file_name("import.json")
    }

    /// Choisit un CSV et ouvre le tampon `*Import*`.
    fn import_csv(&mut self) {
        let Some(from) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv", "txt", "tsv"])
            .pick_file()
        else {
            self.note = Some("Abandon".into());
            return;
        };
        match std::fs::read(&from) {
            Ok(b) => {
                let name = from
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.open_import(&name, &import::decode(&b));
            }
            Err(e) => self.note = Some(format!("{}: {e}", from.display())),
        }
    }

    /// Le choix gardé s'il va à ce fichier, sinon celui qu'on devine — en
    /// gardant les réglages (codes, mots, filtre).
    fn open_import(&mut self, file: &str, text: &str) {
        let table = import::read_table(text);
        if table.rows.is_empty() {
            self.note = Some(format!("{file} : aucune ligne"));
            return;
        }
        let saved: Option<import::Mapping> = std::fs::read_to_string(self.mapping_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
        let mapping = match saved {
            Some(m) if m.fits(&table.heads) => m,
            Some(m) => import::Mapping {
                product_filter: m.product_filter,
                operator_fixed: m.operator_fixed,
                operator_codes: m.operator_codes,
                officine_words: m.officine_words,
                ..import::guess(&table.heads)
            },
            None => import::guess(&table.heads),
        };
        self.import = Some(ImportView {
            file: file.to_owned(),
            table,
            mapping,
            seen: None,
            preview: Vec::new(),
            result: model::Imported::default(),
        });
        self.edit = None;
        self.confirm_delete = None;
    }

    /// Ce que le choix donnerait, sur une copie du registre ; refait quand
    /// le choix change, pas à chaque image.
    fn import_preview(&mut self) {
        let Some(v) = self.import.as_mut() else {
            return;
        };
        if v.seen.as_ref() == Some(&v.mapping) {
            return;
        }
        let mut copy = self.store.clone();
        let before = copy.doses.len();
        let (rows, refused, skipped) = import::doses(&v.table, &v.mapping, &self.today);
        v.result = model::add_imported(&mut copy, rows);
        v.result.refused = refused;
        v.result.skipped = skipped;
        v.preview = copy.doses.split_off(before);
        v.seen = Some(v.mapping.clone());
    }

    fn finish_import(&mut self) {
        let Some(v) = self.import.take() else {
            return;
        };
        let (rows, refused, skipped) = import::doses(&v.table, &v.mapping, &self.today);
        let r = model::add_imported(&mut self.store, rows);
        if r.added > 0 {
            self.save();
        }
        if let Ok(text) = serde_json::to_string_pretty(&v.mapping) {
            let _ = std::fs::write(self.mapping_path(), text);
        }
        let mut said = format!(
            "{} : {} doses ajoutées, {} déjà là",
            v.file, r.added, r.known
        );
        if !refused.is_empty() {
            said.push_str(&format!(", {} refusées", refused.len()));
        }
        if skipped.returns > 0 {
            said.push_str(&format!(", {} retours", skipped.returns));
        }
        self.note = Some(said);
        self.tab = 0;
    }

    /// Le tampon `*Import*` : les colonnes à choisir, les réglages, ce que
    /// l'import ajouterait.
    fn import_view(&mut self, ui: &mut egui::Ui) {
        self.import_preview();
        let c = t::ch(ui);
        let rh = t::row_h(ui);
        let Some(v) = self.import.as_mut() else {
            return;
        };
        let (mut go, mut cancel) = (false, ui.input(|i| i.key_pressed(Key::Escape)));
        t::heading(
            ui,
            "Importer",
            &format!("{}, {} lignes", v.file, v.table.rows.len()),
        );
        let heads = v.table.heads.clone();
        // Deux colonnes de choix, côte à côte quand la place le permet.
        let label_w = 18.0 * c;
        let pick_w = 26.0 * c;
        let per_row = if ui.available_width() >= 2.0 * (label_w + pick_w + 4.0 * c) {
            2
        } else {
            1
        };
        let n = import::COLUMNS.len();
        for start in (0..n).step_by(per_row) {
            ui.horizontal(|ui| {
                ui.set_min_height(rh);
                for i in start..(start + per_row).min(n) {
                    let (label, needed) = import::COLUMNS[i];
                    cell(ui, label_w, |ui| ui.label(t::shadow(label)));
                    let current = v.mapping.column_mut(i);
                    let shown = if current.is_empty() {
                        "-".to_owned()
                    } else {
                        current.clone()
                    };
                    let r = egui::ComboBox::from_id_salt(("imp_col", i))
                        .selected_text(shown)
                        .width(pick_w - 2.0 * c)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(current, String::new(), "-");
                            for h in &heads {
                                ui.selectable_value(current, h.clone(), h);
                            }
                        });
                    if needed && current.is_empty() {
                        t::squiggle(ui, r.response.rect);
                    }
                    ui.add_space(4.0 * c);
                }
            });
        }
        ui.add_space(rh * 0.3);
        let texts: [(&str, &str, &str); 4] = [
            ("Produit contient", "imp_filter", "shingrix"),
            ("Opérateur fixe", "imp_fixed", "si la colonne est vide"),
            ("Codes vendeur", "imp_codes", "03=YS 05=AS"),
            ("Officine si", "imp_off", "mots de la colonne Prescripteur"),
        ];
        for (i, (label, id, hint)) in texts.into_iter().enumerate() {
            ui.horizontal(|ui| {
                cell(ui, label_w, |ui| ui.label(t::shadow(label)));
                let buf = match i {
                    0 => &mut v.mapping.product_filter,
                    1 => &mut v.mapping.operator_fixed,
                    2 => &mut v.mapping.operator_codes,
                    _ => &mut v.mapping.officine_words,
                };
                let w = (ui.available_width() - c).clamp(20.0 * c, 60.0 * c);
                t::field(ui, id, buf, w, hint);
            });
        }
        ui.add_space(rh * 0.3);
        // Le compte, puis les boutons.
        let r = &v.result;
        let mut said = format!("{} à ajouter", r.added);
        for (k, one, many) in [
            (r.known, "déjà là", "déjà là"),
            (r.refused.len(), "refusée", "refusées"),
            (r.skipped.other, "autre produit", "autres produits"),
            (r.skipped.returns, "retour", "retours"),
        ] {
            if k > 0 {
                said.push_str(&format!(", {k} {}", if k == 1 { one } else { many }));
            }
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(said).strong());
            ui.add_space(2.0 * c);
            go |= ui
                .add_enabled_ui(!self.locked && r.added > 0, |ui| t::button(ui, "Importer"))
                .inner
                .clicked();
            ui.add_space(c);
            cancel |= t::button(ui, "Annuler").clicked();
        });
        for (line, why) in r.refused.iter().take(3) {
            ui.label(egui::RichText::new(format!("ligne {line} : {why}")).color(t::ERROR));
        }
        if r.refused.len() > 3 {
            ui.label(t::shadow(format!("… et {} autres", r.refused.len() - 3)));
        }
        // Le fichier tel qu'il est, ses premières lignes : de quoi choisir.
        t::heading(ui, "Fichier", "les premières lignes");
        let shown: Vec<&Vec<String>> = v.table.rows.iter().take(4).map(|r| &r.1).collect();
        let widths: Vec<f32> = heads
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let longest = shown
                    .iter()
                    .filter_map(|r| r.get(i))
                    .chain([h])
                    .map(|s| s.chars().count())
                    .max()
                    .unwrap_or(0);
                (longest.clamp(4, 24) + 2) as f32 * c
            })
            .collect();
        egui::ScrollArea::horizontal()
            .id_salt("import_file")
            .show(ui, |ui| {
                let total: f32 = widths.iter().sum::<f32>() + 2.0 * c;
                ui.set_min_width(total);
                let names: Vec<&str> = heads.iter().map(String::as_str).collect();
                ui.allocate_ui(Vec2::new(total, rh), |ui| {
                    t::header_line(ui, &widths, &names, c)
                });
                for r in &shown {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, rh), Sense::hover());
                    let cells: Vec<Cell> = (0..heads.len())
                        .map(|i| cl(r.get(i).cloned().unwrap_or_default()))
                        .collect();
                    paint_row(ui, rect, &widths, &cells);
                }
            });
        // L'aperçu : les doses telles qu'elles entreraient au registre.
        t::heading(ui, "Aperçu", "ce que l'import ajouterait");
        let cols = register_columns(ui);
        t::header_line(
            ui,
            &cols,
            &[
                "Date", "Dose", "Patient", "Âge", "DDN", "Opér.", "Prescr.", "",
            ],
            c,
        );
        let none = || cl("-").color(t::SHADOW);
        egui::ScrollArea::vertical()
            .id_salt("import_preview")
            .auto_shrink([false, false])
            .show_rows(ui, rh, v.preview.len(), |ui, range| {
                for d in &v.preview[range] {
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::hover());
                    let cells = [
                        cl(model::french(&d.date)),
                        cl(if d.rank == 2 { "D2" } else { "D1" }),
                        if d.name.is_empty() {
                            none()
                        } else {
                            cl(d.name.clone())
                        },
                        model::dose_age(d).map_or_else(none, |a| cl(a.to_string())),
                        if d.birth.is_empty() {
                            none()
                        } else {
                            cl(model::french(&d.birth))
                        },
                        cl(d.operator.clone()).bold(),
                        match d.prescriber {
                            model::Prescriber::Inconnu => none(),
                            model::Prescriber::Medecin => cl("méd.").color(t::SHADOW),
                            model::Prescriber::Officine => cl("off."),
                        },
                        cl(""),
                    ];
                    paint_row(ui, rect, &cols, &cells);
                }
            });
        if cancel {
            self.import = None;
            self.note = Some("Import abandonné".into());
        } else if go {
            self.finish_import();
        }
    }

    fn export_csv(&mut self) {
        let Some(to) = rfd::FileDialog::new()
            .set_file_name(format!("shingrix-{}.csv", self.today))
            .save_file()
        else {
            self.note = Some("Abandon".into());
            return;
        };
        let mut out = String::from("date;dose;patient;age;ddn;operateur;prescripteur\n");
        let mut rows: Vec<&Dose> = self.store.doses.iter().collect();
        rows.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
        for d in rows {
            out.push_str(&format!(
                "{};{};{};{};{};{};{}\n",
                model::french(&d.date),
                d.rank,
                d.name.replace(';', " "),
                model::dose_age(d)
                    .map(|a| a.to_string())
                    .unwrap_or_default(),
                model::french(&d.birth),
                d.operator,
                d.prescriber.word()
            ));
        }
        self.note = Some(match std::fs::write(&to, out) {
            Ok(()) => format!("Écrit {}", to.display()),
            Err(e) => format!("{}: {e}", to.display()),
        });
    }
}

// ---------------------------------------------------------------------
// Graphiques
// ---------------------------------------------------------------------

impl App {
    /// Des barres groupées sur le blanc du tampon : l'axe, des graduations
    /// discrètes, la valeur au-dessus. Le groupe survolé se surligne
    /// (`mouse-face`) et se lit dans la zone d'écho.
    fn bars(
        &mut self,
        ui: &mut egui::Ui,
        groups: &[(String, Vec<(f64, Color32)>)],
        lines: f32,
        names: &[&str],
    ) {
        let c = t::ch(ui);
        let height = lines * t::row_h(ui);
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        let p = ui.painter_at(rect);
        let small = egui::FontId::monospace(t::SIZE - 2.0);
        let plot = egui::Rect::from_min_max(
            rect.min + Vec2::new(5.0 * c, t::line(ui)),
            rect.max - Vec2::new(c, t::line(ui) + 6.0),
        );
        let peak = groups
            .iter()
            .flat_map(|g| g.1.iter().map(|v| v.0))
            .fold(0.0, f64::max)
            .max(1.0);
        let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]
            .into_iter()
            .find(|s| peak / s <= 5.0)
            .unwrap_or(200.0);
        let top = (peak / step).ceil() * step;
        let n = groups.len().max(1);
        let slot = plot.width() / n as f32;
        let hovered = resp
            .hover_pos()
            .filter(|h| plot.x_range().contains(h.x))
            .map(|h| (((h.x - plot.left()) / slot) as usize).min(n - 1))
            .filter(|_| !groups.is_empty());
        if let Some(i) = hovered {
            let x = plot.left() + slot * i as f32;
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x, plot.top()),
                    egui::pos2(x + slot, rect.bottom()),
                ),
                0.0,
                t::HIGHLIGHT,
            );
        }
        for i in 0..=((top / step) as usize) {
            let v = i as f64 * step;
            let y = (plot.bottom() - plot.height() * (v / top) as f32).round() + 0.5;
            if i > 0 {
                p.line_segment(
                    [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                    (1.0, t::GREY90),
                );
            }
            p.text(
                egui::pos2(plot.left() - c, y),
                egui::Align2::RIGHT_CENTER,
                format!("{v:.0}"),
                small.clone(),
                t::SHADOW,
            );
        }
        let k = groups.first().map_or(1, |g| g.1.len()).max(1);
        let bw = ((slot * 0.7) / k as f32).min(3.0 * c);
        let label_every = ((n as f32 * 7.0 * c) / plot.width()).ceil().max(1.0) as usize;
        for (i, (label, vals)) in groups.iter().enumerate() {
            let cx = plot.left() + slot * (i as f32 + 0.5);
            let x0 = cx - bw * k as f32 / 2.0;
            for (j, (v, col)) in vals.iter().enumerate() {
                if *v <= 0.0 {
                    continue;
                }
                let h = plot.height() * (*v / top) as f32;
                let r = egui::Rect::from_min_max(
                    egui::pos2((x0 + j as f32 * bw).round(), plot.bottom() - h),
                    egui::pos2((x0 + (j + 1) as f32 * bw).round() - 1.0, plot.bottom()),
                );
                p.rect_filled(r, 0.0, *col);
                if bw >= 1.6 * c {
                    p.text(
                        egui::pos2(r.center().x, r.top() - 1.0),
                        egui::Align2::CENTER_BOTTOM,
                        format!("{v:.0}"),
                        small.clone(),
                        t::FG,
                    );
                }
            }
            if i % label_every == 0 {
                p.text(
                    egui::pos2(cx, plot.bottom() + 4.0),
                    egui::Align2::CENTER_TOP,
                    label,
                    small.clone(),
                    t::FG,
                );
            }
        }
        let y = plot.bottom().round() + 0.5;
        p.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            (1.0, t::FG),
        );
        if let Some(i) = hovered {
            let (label, vals) = &groups[i];
            let said: Vec<String> = vals
                .iter()
                .enumerate()
                .map(|(j, v)| match names.get(j) {
                    Some(nm) => format!("{nm} {:.0}", v.0),
                    None => format!("{:.0}", v.0),
                })
                .collect();
            self.hover = Some(format!("{label} : {}", said.join(", ")));
        }
    }
}

fn legend(ui: &mut egui::Ui, items: &[(&str, Color32)]) {
    let c = t::ch(ui);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(5.0 * c);
        for (l, col) in items {
            ui.label(RichText::new("■").color(*col));
            ui.label(format!(" {l}"));
            ui.add_space(2.0 * c);
        }
    });
}

/// Des barres horizontales en caractères, au huitième près — à la
/// manière des sorties texte d'Emacs.
fn text_bars(ui: &mut egui::Ui, rows: &[(String, usize, Color32)], of: usize) {
    const EIGHTHS: [&str; 8] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];
    let c = t::ch(ui);
    let rh = t::row_h(ui);
    let label_w = 8.0 * c;
    let tail_w = 11.0 * c;
    let cells = ((ui.available_width() - label_w - tail_w - c) / c)
        .floor()
        .max(4.0) as usize;
    let peak = rows.iter().map(|r| r.1).max().unwrap_or(0).max(1);
    for (label, n, col) in rows {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::hover());
        let p = ui.painter();
        let y = rect.center().y;
        p.text(
            egui::pos2(rect.left() + c, y),
            egui::Align2::LEFT_CENTER,
            label,
            t::mono(),
            t::FG,
        );
        let eighths = (*n * cells * 8).div_ceil(peak).min(cells * 8);
        let eighths = if *n > 0 { eighths.max(1) } else { 0 };
        let bar = format!("{}{}", "█".repeat(eighths / 8), EIGHTHS[eighths % 8]);
        p.text(
            egui::pos2(rect.left() + label_w, y),
            egui::Align2::LEFT_CENTER,
            bar,
            t::mono(),
            *col,
        );
        let x = rect.left() + label_w + cells as f32 * c;
        p.text(
            egui::pos2(x + 4.0 * c, y),
            egui::Align2::RIGHT_CENTER,
            n.to_string(),
            t::bold_font(),
            t::FG,
        );
        p.text(
            egui::pos2(x + 10.0 * c, y),
            egui::Align2::RIGHT_CENTER,
            model::pct((of > 0).then(|| *n as f64 / of as f64)),
            t::mono(),
            t::SHADOW,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dose(date: &str, rank: u8, name: &str, birth: &str, op: &str) -> Dose {
        Dose {
            id: 0,
            date: date.into(),
            rank,
            name: name.into(),
            age: None,
            birth: birth.into(),
            operator: op.into(),
            // Un sur cinq sur ordonnance d'un médecin.
            prescriber: if date.ends_with(['0', '5']) {
                model::Prescriber::Medecin
            } else {
                model::Prescriber::Officine
            },
        }
    }

    /// Chaque onglet, chaque scène, trois tailles, une base vide et une
    /// pleine, et les périodes : la fenêtre se dessine sans paniquer.
    #[test]
    fn every_tab_draws_at_every_size() {
        let dir = std::env::temp_dir().join(format!("bilan-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let full = dir.join("doses.json");
        let mut store = Store::default();
        for (i, d) in [
            dose("2026-03-02", 1, "Martin Roger", "1947-10-28", "YS"),
            dose("2026-05-04", 2, "Martin Roger", "", "as"),
            dose("2026-10-30", 1, "", "", "CL"),
            dose("2026-07-15", 1, "Dubois Paul", "", "YS"),
            dose("2025-12-13", 1, "Girard Marie", "1966-04-05", ""),
        ]
        .into_iter()
        .enumerate()
        {
            let mut d = d;
            d.age = (i == 3).then_some(88);
            store.add(d);
        }
        store.save(&full).unwrap();
        let empty = dir.join("vide.json");
        for path in [full, empty] {
            for size in [[980.0, 620.0], [1200.0, 780.0], [1920.0, 1080.0]] {
                for tab in 0..TABS.len() {
                    for scene in ["", "connus", "modifier", "supprimer", "import"] {
                        for period in Period::ALL {
                            let ctx = egui::Context::default();
                            t::apply(&ctx);
                            let mut app = App::new(path.clone());
                            app.tab = tab;
                            app.period = period;
                            app.set_scene(scene);
                            // Le pointeur balaie la fenêtre : survols des
                            // graphiques et des lignes.
                            for k in 0..6 {
                                let at = egui::pos2(
                                    size[0] * (k as f32 + 0.5) / 6.0,
                                    size[1] * (0.2 + 0.12 * k as f32),
                                );
                                let input = egui::RawInput {
                                    screen_rect: Some(egui::Rect::from_min_size(
                                        egui::Pos2::ZERO,
                                        size.into(),
                                    )),
                                    events: vec![egui::Event::PointerMoved(at)],
                                    ..Default::default()
                                };
                                let _ = ctx.run(input, |ctx| app.frame(ctx));
                            }
                        }
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn key(k: Key) -> egui::Event {
        egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn run(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 780.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.frame(ctx));
    }

    /// Au clavier : un nom, Entrée, la dose est au registre ; puis la
    /// question de la zone d'écho, « n » la laisse, « o » la supprime.
    #[test]
    fn an_import_is_previewed_then_added_and_its_choice_kept() {
        let dir = std::env::temp_dir().join(format!("bilan-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doses.json");
        let ctx = egui::Context::default();
        t::apply(&ctx);
        let mut app = App::new(path.clone());
        app.open_import("exemple.csv", import::SAMPLE);
        run(&ctx, &mut app, vec![]);
        // Sans codes vendeur, aucune ligne n'a d'initiales.
        assert_eq!(app.import.as_ref().unwrap().result.added, 0);
        app.import.as_mut().unwrap().mapping.operator_codes = "03=YS 05=AS".into();
        run(&ctx, &mut app, vec![]);
        let v = app.import.as_ref().unwrap();
        assert_eq!((v.result.added, v.preview.len()), (2, 2));
        assert!(
            app.store.doses.is_empty(),
            "l'aperçu ne touche pas au registre"
        );
        app.finish_import();
        assert!(app.import.is_none());
        assert_eq!(Store::load(&path).unwrap().doses.len(), 2);
        // Le fichier suivant reprend le choix gardé ; rien n'est en double.
        app.open_import("exemple.csv", import::SAMPLE);
        assert_eq!(
            app.import.as_ref().unwrap().mapping.operator_codes,
            "03=YS 05=AS"
        );
        run(&ctx, &mut app, vec![]);
        assert_eq!(app.import.as_ref().unwrap().result.known, 2);
        run(&ctx, &mut app, vec![key(Key::Escape)]);
        assert!(app.import.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_keyboard_enters_and_deletes_a_dose() {
        let dir = std::env::temp_dir().join(format!("bilan-keys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doses.json");
        let ctx = egui::Context::default();
        t::apply(&ctx);
        let mut app = App::new(path.clone());
        // Sans opérateur, rien n'entre et la zone d'écho le dit.
        run(&ctx, &mut app, vec![]);
        run(&ctx, &mut app, vec![egui::Event::Text("Martin".into())]);
        run(&ctx, &mut app, vec![key(Key::Enter)]);
        assert!(app.store.doses.is_empty());
        assert_eq!(
            app.note.as_deref(),
            Some("Opérateur manquant : ses initiales")
        );
        app.entry.name.clear();
        app.focus = Some("in_name");
        app.entry.operator = "ys".into();
        run(&ctx, &mut app, vec![]);
        run(
            &ctx,
            &mut app,
            vec![egui::Event::Text("Martin Roger".into())],
        );
        assert_eq!(app.entry.name, "Martin Roger");
        run(&ctx, &mut app, vec![key(Key::Enter)]);
        run(&ctx, &mut app, vec![]);
        assert_eq!(app.store.doses.len(), 1, "{:?}", app.note);
        let d = &app.store.doses[0];
        assert_eq!((d.rank, d.operator.as_str()), (1, "YS"));
        assert_eq!(Store::load(&path).unwrap().doses.len(), 1);
        // Le nom est vidé, la date et l'opérateur restent.
        assert!(app.entry.name.is_empty());
        assert_eq!(app.entry.operator, "YS");

        app.set_scene("supprimer");
        run(&ctx, &mut app, vec![]);
        assert!(app.delete_prompt().is_some());
        run(&ctx, &mut app, vec![key(Key::N)]);
        assert!(app.confirm_delete.is_none());
        assert_eq!(app.store.doses.len(), 1);
        app.set_scene("supprimer");
        run(
            &ctx,
            &mut app,
            vec![egui::Event::Text("o".into()), key(Key::O)],
        );
        assert!(app.store.doses.is_empty());
        assert!(Store::load(&path).unwrap().doses.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
