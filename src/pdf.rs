//! Le bilan en PDF, par Typst, dans le programme : ni installation, ni
//! réseau. Il a la tenue d'un manuel GNU imprimé par Texinfo. Le modèle
//! est `src/report.typ` ; ce module n'écrit que ses
//! données, en `#let` en tête. **Aucun nom de patient n'y entre** : des
//! comptes, et les initiales de l'équipe.

use crate::model::{self, Due, Pending, Stats, BANDS};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

const TEMPLATE: &str = include_str!("report.typ");

/// New Computer Modern : le Computer Modern des manuels Texinfo.
const FONTS: [&[u8]; 3] = [
    include_bytes!("../fonts/NewCM10-Regular.otf"),
    include_bytes!("../fonts/NewCM10-Bold.otf"),
    include_bytes!("../fonts/NewCM10-Italic.otf"),
];

pub struct Head {
    /// « 01/12/2025 – 04/11/2026 ».
    pub period: String,
    pub edited: String,
}

/// Une chaîne Typst.
fn s(v: &str) -> String {
    let mut out = String::from("\"");
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Un tableau Typst : la virgule finale d'un tableau d'un élément.
fn array(items: Vec<String>) -> String {
    let one = items.len() == 1;
    format!("({}{})", items.join(", "), if one { "," } else { "" })
}

/// Les huit chiffres clés : (valeur, mot, précision, couleur) — les mêmes
/// que l'onglet « Bilan ».
pub fn tiles(st: &Stats, pending: &[Pending], expected: &[(String, usize)]) -> Vec<[String; 4]> {
    let count = |d: Due| pending.iter().filter(|p| p.due == d).count();
    let t = |v: String, l: &str, sub: String, c: &str| [v, l.to_owned(), sub, c.to_owned()];
    vec![
        t(
            st.total().to_string(),
            "Doses",
            format!("D1 {}, D2 {}", st.d1, st.d2),
            "slate",
        ),
        t(st.people.to_string(), "Patients", String::new(), "slate"),
        t(
            model::pct(st.completion()),
            "Complets",
            format!("{}/{}", st.completed, st.started),
            "green",
        ),
        t(
            st.mean_age
                .map(|a| format!("{} ans", a.round() as i64))
                .unwrap_or_else(|| "—".into()),
            "Âge",
            "moyen".into(),
            "slate",
        ),
        t(
            model::months_said(st.median_gap),
            "Délai",
            "D1 → D2".into(),
            "amber",
        ),
        t(
            format!("{}/{}", st.in_window, st.gaps),
            "Fenêtre",
            "2–6 mois".into(),
            "amber",
        ),
        t(
            (count(Due::Now) + count(Due::Late)).to_string(),
            "Dues",
            format!("retard {}", count(Due::Late)),
            "red",
        ),
        t(
            expected.iter().map(|e| e.1).sum::<usize>().to_string(),
            "À venir",
            "6 mois".into(),
            "amber",
        ),
    ]
}

/// Une ligne de la comparaison : les neuf cellules après les initiales.
pub fn team_cells(o: &model::OpRow, total: usize) -> [String; 9] {
    [
        o.doses.to_string(),
        model::pct((total > 0).then(|| o.doses as f64 / total as f64)),
        o.d1.to_string(),
        o.d2.to_string(),
        o.patients.to_string(),
        o.mean_age
            .map(|a| format!("{} ans", a.round() as i64))
            .unwrap_or_else(|| "—".into()),
        model::months_said(o.median_gap),
        if o.gaps == 0 {
            "—".into()
        } else {
            format!("{}/{}", o.in_window, o.gaps)
        },
        model::french(&o.last),
    ]
}

pub fn source(
    st: &Stats,
    pending: &[Pending],
    expected: &[(String, usize)],
    head: &Head,
) -> String {
    let months = array(
        st.months
            .iter()
            .map(|(m, a, b)| format!("({}, ({a}, {b}))", s(&model::month_label(m))))
            .collect(),
    );
    let waiting = array(
        expected
            .iter()
            .map(|(m, n)| format!("({}, ({n},))", s(&model::month_label(m))))
            .collect(),
    );
    let ages = array(
        BANDS
            .iter()
            .map(|b| b.2.to_owned())
            .chain(["?".to_owned()])
            .zip(st.ages)
            .filter(|(l, n)| *n > 0 || l != "?")
            .map(|(l, n)| format!("({}, {n})", s(&l)))
            .collect(),
    );
    let tuiles = array(
        tiles(st, pending, expected)
            .iter()
            .map(|t| format!("({}, {}, {}, {})", s(&t[0]), s(&t[1]), s(&t[2]), s(&t[3])))
            .collect(),
    );
    let total = st.total();
    let team = array(
        st.team
            .iter()
            .map(|o| {
                let cells: Vec<String> = team_cells(o, total).iter().map(|c| s(c)).collect();
                format!("({}, {}, {})", s(&o.key), array(cells), o.doses)
            })
            .collect(),
    );
    let team_months = array(
        st.months
            .iter()
            .enumerate()
            .map(|(i, (m, _, _))| {
                let vals: Vec<String> = st.team.iter().map(|o| o.months[i].to_string()).collect();
                format!("({}, {})", s(&model::month_label(m)), array(vals))
            })
            .collect(),
    );
    let data = [
        format!("#let periode = {}", s(&head.period)),
        format!("#let edite = {}", s(&head.edited)),
        format!("#let total = {total}"),
        format!("#let tuiles = {tuiles}"),
        format!("#let mois = {months}"),
        format!("#let attente = {waiting}"),
        format!("#let ages = {ages}"),
        format!("#let equipe = {team}"),
        format!("#let equipe_mois = {team_months}"),
    ]
    .join("\n");
    TEMPLATE.replacen("// {{DATA}}", &data, 1)
}

struct ReportWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    source: Source,
}

fn fonts() -> &'static (Vec<Font>, FontBook) {
    static F: std::sync::OnceLock<(Vec<Font>, FontBook)> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let fonts: Vec<Font> = FONTS
            .iter()
            .flat_map(|d| Font::iter(Bytes::new(*d)))
            .collect();
        let book = FontBook::from_fonts(&fonts);
        (fonts, book)
    })
}

impl World for ReportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.source.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

pub fn render(
    st: &Stats,
    pending: &[Pending],
    expected: &[(String, usize)],
    head: &Head,
) -> Result<Vec<u8>, String> {
    let (fonts, book) = fonts();
    let world = ReportWorld {
        library: LazyHash::new(Library::default()),
        book: LazyHash::new(book.clone()),
        fonts: fonts.clone(),
        source: Source::detached(source(st, pending, expected, head)),
    };
    let join = |errs: &[typst::diag::SourceDiagnostic]| {
        errs.iter()
            .map(|e| e.message.to_string())
            .collect::<Vec<_>>()
            .join(" ; ")
    };
    let doc: PagedDocument = typst::compile(&world).output.map_err(|e| join(&e))?;
    typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).map_err(|e| join(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn year(n: usize, ops: &[&str]) -> Vec<Dose> {
        let mut seed = 11u32;
        let mut rnd = |m: u32| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) % m
        };
        let mut out = Vec::new();
        for i in 0..n {
            let m = 1 + rnd(10);
            let date = format!("2026-{m:02}-{:02}", 1 + rnd(28));
            let birth = format!("19{}-0{}-1{}", 35 + rnd(30), 1 + rnd(9), rnd(9));
            let op = ops[rnd(ops.len() as u32) as usize].to_owned();
            out.push(Dose {
                id: 0,
                date: date.clone(),
                rank: 1,
                name: format!("Zorglub {i}"),
                age: None,
                birth: birth.clone(),
                operator: op.clone(),
            });
            if rnd(3) > 0 {
                let d2 = add_months(&date, 2 + rnd(4) as i32);
                if d2.as_str() <= "2026-11-04" {
                    let op = ops[rnd(ops.len() as u32) as usize].to_owned();
                    out.push(Dose {
                        id: 0,
                        date: d2,
                        rank: 2,
                        name: format!("Zorglub {i}"),
                        age: None,
                        birth,
                        operator: op,
                    });
                }
            }
        }
        out
    }

    fn build(doses: &[Dose], from: &str) -> Vec<u8> {
        let today = "2026-11-04";
        let st = stats(doses, from, today);
        let pend = pending(&patients(doses), today);
        let exp = expected_by_month(&pend, today, 6);
        let head = Head {
            period: "01/12/2025 – 04/11/2026".into(),
            edited: "04/11/2026".into(),
        };
        render(&st, &pend, &exp, &head).unwrap_or_else(|e| panic!("{from:?}: {e}"))
    }

    /// Vide, un opérateur, huit (plus que la palette), un mois, tout : il
    /// se compile, et aucun nom de patient n'y entre.
    #[test]
    fn the_report_compiles_and_names_no_patient() {
        assert!(build(&[], "2026-11-01").starts_with(b"%PDF"));
        for ops in [
            &["YS"][..],
            &["YS", "AS", "CL"],
            &["A", "B", "C", "D", "E", "F", "G", "H"],
        ] {
            let doses = year(60, ops);
            for from in ["2026-11-01", "2025-12-01", ""] {
                assert!(build(&doses, from).starts_with(b"%PDF"));
            }
        }
        let doses = year(60, &["YS"]);
        let st = stats(&doses, "", "2026-11-04");
        let pend = pending(&patients(&doses), "2026-11-04");
        let exp = expected_by_month(&pend, "2026-11-04", 6);
        let head = Head {
            period: String::new(),
            edited: String::new(),
        };
        assert!(!source(&st, &pend, &exp, &head).contains("Zorglub"));
    }

    /// `BILAN_SAMPLE_PDF=/chemin.pdf cargo test sample` : un bilan à regarder.
    #[test]
    fn sample() {
        let Some(out) = std::env::var_os("BILAN_SAMPLE_PDF") else {
            return;
        };
        // `BILAN_SAMPLE_OPS=A,B,C` et `BILAN_SAMPLE_N=0` pour d'autres cas.
        let ops = std::env::var("BILAN_SAMPLE_OPS").unwrap_or_else(|_| "YS,AS,CL".into());
        let ops: Vec<&str> = ops.split(',').collect();
        let n = std::env::var("BILAN_SAMPLE_N")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(70);
        std::fs::write(out, build(&year(n, &ops), "2025-12-01")).unwrap();
    }
}
