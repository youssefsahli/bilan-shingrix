//! Les doses, leur fichier, les patients qu'on en déduit, le bilan et les
//! prévisions. Pur : pas d'egui, pas d'horloge (le jour est passé).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Shingrix : la 2e dose de 2 à 6 mois après la 1re (RCP).
pub const D2_FROM_MONTHS: i32 = 2;
pub const D2_TO_MONTHS: i32 = 6;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dose {
    pub id: u64,
    /// ISO `AAAA-MM-JJ`.
    pub date: String,
    /// 1 ou 2.
    pub rank: u8,
    pub name: String,
    pub age: Option<u8>,
    /// ISO, ou vide.
    pub birth: String,
    /// Les initiales de qui a injecté, en capitales (« AS », « YS »).
    pub operator: String,
    pub prescriber: Prescriber,
}

/// Qui a prescrit la dose : l'officine (ce qu'on veut compter) ou un
/// médecin. Inconnu pour un registre d'avant ou un import qui ne le dit pas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Prescriber {
    #[default]
    Inconnu,
    Officine,
    Medecin,
}

impl Prescriber {
    /// Le mot du registre.
    pub fn short(self) -> &'static str {
        match self {
            Prescriber::Inconnu => "?",
            Prescriber::Officine => "off.",
            Prescriber::Medecin => "méd.",
        }
    }
    /// Le mot du CSV.
    pub fn word(self) -> &'static str {
        match self {
            Prescriber::Inconnu => "",
            Prescriber::Officine => "officine",
            Prescriber::Medecin => "medecin",
        }
    }
    /// `officine`, `off`, `pharmacien`, `o` ; `medecin`, `méd`, `m` ; vide.
    pub fn read(s: &str) -> Option<Prescriber> {
        let f = fold(s);
        let f = f.trim_end_matches('.');
        match f {
            "" | "?" => Some(Prescriber::Inconnu),
            _ if ["officine", "off", "o", "pharmacien", "pharmacie", "ph"].contains(&f) => {
                Some(Prescriber::Officine)
            }
            _ if ["medecin", "med", "m", "dr"].contains(&f) => Some(Prescriber::Medecin),
            _ => None,
        }
    }
    /// L'ordre du bouton de la ligne en cours de modification.
    pub fn next(self) -> Prescriber {
        match self {
            Prescriber::Officine => Prescriber::Medecin,
            Prescriber::Medecin => Prescriber::Inconnu,
            Prescriber::Inconnu => Prescriber::Officine,
        }
    }
}

/// Les initiales d'un opérateur, lues sans égard à la casse : de 1 à 5
/// lettres, rendues en capitales. `None` : vide ou autre chose que des
/// lettres.
pub fn operator_key(typed: &str) -> Option<String> {
    let k: String = typed.trim().chars().flat_map(char::to_uppercase).collect();
    let n = k.chars().count();
    ((1..=5).contains(&n) && k.chars().all(char::is_alphabetic)).then_some(k)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub doses: Vec<Dose>,
    pub next_id: u64,
}

impl Store {
    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bilan-shingrix")
            .join("doses.json")
    }

    /// Absent : un registre vide. Illisible : l'erreur — jamais un registre
    /// vide qui écraserait le fichier à la première saisie.
    pub fn load(path: &Path) -> Result<Store, String> {
        match std::fs::read_to_string(path) {
            Ok(t) => {
                serde_json::from_str(&t).map_err(|e| format!("{} illisible : {e}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
            Err(e) => Err(format!("{} : {e}", path.display())),
        }
    }

    /// Écrit à côté puis remplace : une coupure laisse l'ancien fichier entier.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    pub fn add(&mut self, mut d: Dose) -> u64 {
        self.next_id = self
            .next_id
            .max(self.doses.iter().map(|d| d.id).max().unwrap_or(0))
            + 1;
        d.id = self.next_id;
        self.doses.push(d);
        self.next_id
    }

    pub fn replace(&mut self, d: Dose) -> bool {
        match self.doses.iter_mut().find(|x| x.id == d.id) {
            Some(x) => {
                *x = d;
                true
            }
            None => false,
        }
    }

    pub fn remove(&mut self, id: u64) -> Option<Dose> {
        let i = self.doses.iter().position(|d| d.id == id)?;
        Some(self.doses.remove(i))
    }
}

// ---------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------

/// `JJ/MM/AAAA`, `JJ/MM/AA`, `JJ/MM` (année en cours), `JJMMAA`,
/// `JJMMAAAA`, `JJMM` ou ISO → ISO. Vide → `None`.
pub fn parse_date(input: &str, today: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if s.len() == 10 && s.as_bytes()[4] == b'-' {
        return valid(
            s[..4].parse().ok()?,
            s[5..7].parse().ok()?,
            s[8..].parse().ok()?,
        );
    }
    let year_now: i32 = today.get(..4)?.parse().ok()?;
    let parts: Vec<&str> = if s.contains(['/', '.', '-', ' ']) {
        s.split(['/', '.', '-', ' '])
            .filter(|p| !p.is_empty())
            .collect()
    } else if s.chars().all(|c| c.is_ascii_digit()) {
        match s.len() {
            4 => vec![&s[..2], &s[2..]],
            6 | 8 => vec![&s[..2], &s[2..4], &s[4..]],
            _ => return None,
        }
    } else {
        return None;
    };
    let d: u32 = parts.first()?.parse().ok()?;
    let m: u32 = parts.get(1)?.parse().ok()?;
    let y: i32 = match parts.get(2) {
        None => year_now,
        // Deux chiffres : le siècle qui ne met pas la date dans le futur
        // (une DDN « 03/04/52 » est 1952).
        Some(y) if y.len() == 2 => {
            let yy: i32 = y.parse().ok()?;
            if 2000 + yy > year_now {
                1900 + yy
            } else {
                2000 + yy
            }
        }
        Some(y) if y.len() == 4 => y.parse().ok()?,
        Some(_) => return None,
    };
    valid(y, m, d)
}

fn valid(y: i32, m: u32, d: u32) -> Option<String> {
    chrono::NaiveDate::from_ymd_opt(y, m, d).map(|d| d.format("%Y-%m-%d").to_string())
}

pub fn french(iso: &str) -> String {
    match (iso.get(..4), iso.get(5..7), iso.get(8..10)) {
        (Some(y), Some(m), Some(d)) => format!("{d}/{m}/{y}"),
        _ => iso.to_owned(),
    }
}

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn ymd(iso: &str) -> Option<(i32, u32, u32)> {
    Some((
        iso.get(..4)?.parse().ok()?,
        iso.get(5..7)?.parse().ok()?,
        iso.get(8..10)?.parse().ok()?,
    ))
}

/// Le même quantième `months` mois plus tard, ramené à la fin du mois.
pub fn add_months(iso: &str, months: i32) -> String {
    let Some((y, m, d)) = ymd(iso) else {
        return iso.to_owned();
    };
    let idx = y * 12 + (m as i32 - 1) + months;
    let (ny, nm) = (idx.div_euclid(12), idx.rem_euclid(12) as u32 + 1);
    let mut day = d;
    while day > 28 && chrono::NaiveDate::from_ymd_opt(ny, nm, day).is_none() {
        day -= 1;
    }
    valid(ny, nm, day).unwrap_or_else(|| iso.to_owned())
}

pub fn days_between(a: &str, b: &str) -> Option<i64> {
    let p = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    Some((p(b)? - p(a)?).num_days())
}

pub fn age_on(birth: &str, day: &str) -> Option<u8> {
    let (by, bm, bd) = ymd(birth)?;
    let (ty, tm, td) = ymd(day)?;
    let mut a = ty - by;
    if (tm, td) < (bm, bd) {
        a -= 1;
    }
    u8::try_from(a).ok()
}

const MONTHS: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
    "déc.",
];

/// `AAAA-MM` → « nov. 26 ».
pub fn month_label(key: &str) -> String {
    let m: usize = key.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
    format!(
        "{} {}",
        MONTHS[m.clamp(1, 12) - 1],
        key.get(2..4).unwrap_or("")
    )
}

fn month_key(iso: &str) -> String {
    iso.get(..7).unwrap_or(iso).to_owned()
}

// ---------------------------------------------------------------------
// Patients
// ---------------------------------------------------------------------

/// Un nom comparable : minuscules, sans accents, espaces simples.
pub fn fold(name: &str) -> String {
    let mut out = String::new();
    for w in name.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        for c in w.chars().flat_map(char::to_lowercase) {
            out.push(match c {
                'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'î' | 'ï' | 'í' => 'i',
                'ô' | 'ö' | 'ó' => 'o',
                'ù' | 'û' | 'ü' | 'ú' => 'u',
                'ç' => 'c',
                'ÿ' => 'y',
                '-' | '\'' | '’' => ' ',
                c => c,
            });
        }
    }
    out
}

/// Un patient : les doses qui portent le même nom (et la même DDN quand
/// les deux en ont une).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patient {
    pub name: String,
    pub birth: String,
    /// Dates des 1res et 2es doses, dans l'ordre.
    pub d1: Vec<String>,
    pub d2: Vec<String>,
    /// L'âge le plus récent qu'on sache (DDN, sinon saisi).
    pub age: Option<u8>,
    pub last_age_day: String,
}

impl Patient {
    pub fn complete(&self) -> bool {
        !self.d2.is_empty()
    }
}

pub fn patients(doses: &[Dose]) -> Vec<Patient> {
    let mut order: Vec<&Dose> = doses.iter().filter(|d| !d.name.trim().is_empty()).collect();
    order.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
    let mut out: Vec<(String, Patient)> = Vec::new();
    for d in order {
        let key = fold(&d.name);
        let found = out.iter_mut().find(|(k, p)| {
            *k == key && (d.birth.is_empty() || p.birth.is_empty() || p.birth == d.birth)
        });
        let p = match found {
            Some((_, p)) => p,
            None => {
                out.push((
                    key,
                    Patient {
                        name: d.name.trim().to_owned(),
                        ..Patient::default()
                    },
                ));
                &mut out.last_mut().expect("pushed").1
            }
        };
        if p.birth.is_empty() && !d.birth.is_empty() {
            p.birth = d.birth.clone();
        }
        if d.rank == 2 {
            p.d2.push(d.date.clone())
        } else {
            p.d1.push(d.date.clone())
        }
        if let Some(a) = dose_age(d) {
            p.age = Some(a);
            p.last_age_day = d.date.clone();
        }
    }
    out.into_iter().map(|(_, p)| p).collect()
}

/// L'âge à la dose : par la DDN quand elle est là, sinon l'âge saisi.
pub fn dose_age(d: &Dose) -> Option<u8> {
    if !d.birth.is_empty() {
        if let Some(a) = age_on(&d.birth, &d.date) {
            return Some(a);
        }
    }
    d.age
}

/// Les patients dont le nom commence par (ou contient) `typed`.
pub fn suggest<'a>(people: &'a [Patient], typed: &str, max: usize) -> Vec<&'a Patient> {
    let q = fold(typed);
    if q.chars().count() < 2 {
        return Vec::new();
    }
    let mut hits: Vec<(bool, &Patient)> = people
        .iter()
        .filter_map(|p| {
            let k = fold(&p.name);
            if k.starts_with(&q) || k.split(' ').any(|w| w.starts_with(&q)) {
                Some((true, p))
            } else if k.contains(&q) {
                Some((false, p))
            } else {
                None
            }
        })
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    hits.into_iter().take(max).map(|(_, p)| p).collect()
}

// ---------------------------------------------------------------------
// Prévisions
// ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Due {
    /// Plus de 6 mois depuis la 1re dose.
    Late,
    /// Entre 2 et 6 mois : à faire maintenant.
    Now,
    /// Moins de 2 mois.
    Soon,
}

impl Due {
    pub fn label(self) -> &'static str {
        match self {
            Due::Late => "Retard",
            Due::Now => "À faire",
            Due::Soon => "Bientôt",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub name: String,
    pub age: Option<u8>,
    pub d1: String,
    pub from: String,
    pub to: String,
    pub due: Due,
    /// Jours jusqu'à l'ouverture (positif) ou depuis la fermeture (négatif).
    pub days: i64,
}

/// Les 2es doses à venir, la plus urgente d'abord.
pub fn pending(people: &[Patient], today: &str) -> Vec<Pending> {
    let mut out: Vec<Pending> = people
        .iter()
        .filter(|p| !p.complete())
        .filter_map(|p| {
            let d1 = p.d1.last()?.clone();
            let from = add_months(&d1, D2_FROM_MONTHS);
            let to = add_months(&d1, D2_TO_MONTHS);
            let (due, days) = if today < from.as_str() {
                (Due::Soon, days_between(today, &from).unwrap_or(0))
            } else if today <= to.as_str() {
                (Due::Now, days_between(today, &to).unwrap_or(0))
            } else {
                (Due::Late, -days_between(&to, today).unwrap_or(0))
            };
            let age = match (&p.birth, p.age) {
                (b, _) if !b.is_empty() => age_on(b, today),
                (_, a) => a,
            };
            Some(Pending {
                name: p.name.clone(),
                age,
                d1,
                from,
                to,
                due,
                days,
            })
        })
        .collect();
    out.sort_by(|a, b| a.due.cmp(&b.due).then_with(|| a.from.cmp(&b.from)));
    out
}

/// Les 2es doses attendues par mois, sur `months` mois à partir du mois
/// en cours : chaque attente compte au mois où sa fenêtre s'ouvre, et
/// celles déjà ouvertes (ou en retard) au mois en cours.
pub fn expected_by_month(p: &[Pending], today: &str, months: i32) -> Vec<(String, usize)> {
    let first = format!("{}-01", &today[..7]);
    let keys: Vec<String> = (0..months)
        .map(|i| month_key(&add_months(&first, i)))
        .collect();
    let mut out: Vec<(String, usize)> = keys.iter().map(|k| (k.clone(), 0)).collect();
    for x in p {
        let k = month_key(&x.from).max(keys[0].clone());
        if let Some(slot) = out.iter_mut().find(|(m, _)| *m == k) {
            slot.1 += 1;
        }
    }
    out
}

// ---------------------------------------------------------------------
// Bilan
// ---------------------------------------------------------------------

pub const BANDS: [(u8, u8, &str); 5] = [
    (0, 49, "< 50"),
    (50, 64, "50–64"),
    (65, 74, "65–74"),
    (75, 84, "75–84"),
    (85, 255, "85 +"),
];

/// Une ligne de la comparaison des opérateurs, sur la période.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpRow {
    /// Les initiales ; « — » pour une dose saisie sans.
    pub key: String,
    pub doses: usize,
    pub d1: usize,
    pub d2: usize,
    /// Doses qu'il a prescrites lui-même, à l'officine.
    pub pharmacy: usize,
    /// Patients nommés distincts.
    pub patients: usize,
    pub mean_age: Option<f64>,
    /// Délai médian 1re → 2e pour les 2es doses qu'il a faites.
    pub median_gap: Option<i64>,
    pub gaps: usize,
    pub in_window: usize,
    /// Doses par mois, dans l'ordre de `Stats::months`.
    pub months: Vec<usize>,
    pub last: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub from: String,
    pub to: String,
    pub d1: usize,
    pub d2: usize,
    /// Doses prescrites à l'officine, sur ordonnance d'un médecin ; le reste
    /// n'a pas de prescripteur connu.
    pub by_pharmacy: usize,
    pub by_doctor: usize,
    /// Personnes distinctes vaccinées dans la période (nommées), plus les
    /// doses sans nom.
    pub people: usize,
    pub anonymous: usize,
    /// (mois, 1res, 2es), chaque mois de la période.
    pub months: Vec<(String, usize, usize)>,
    /// Doses par tranche d'âge à l'injection, puis âge inconnu.
    pub ages: [usize; 6],
    pub mean_age: Option<f64>,
    /// Délai médian 1re → 2e dose (jours), pour les 2es doses de la période.
    pub median_gap: Option<i64>,
    pub gaps: usize,
    /// 2es doses faites dans la fenêtre de 2 à 6 mois.
    pub in_window: usize,
    /// Parmi les personnes ayant reçu une 1re dose (toutes dates) : combien
    /// ont la 2e.
    pub started: usize,
    pub completed: usize,
    /// La comparaison des opérateurs, le plus actif d'abord.
    pub team: Vec<OpRow>,
}

impl Stats {
    pub fn total(&self) -> usize {
        self.d1 + self.d2
    }
    pub fn completion(&self) -> Option<f64> {
        (self.started > 0).then(|| self.completed as f64 / self.started as f64)
    }
}

fn median(mut v: Vec<i64>) -> Option<i64> {
    v.sort_unstable();
    (!v.is_empty()).then(|| v[v.len() / 2])
}

#[derive(Default)]
struct OpAcc {
    d1: usize,
    d2: usize,
    pharmacy: usize,
    patients: std::collections::BTreeSet<String>,
    ages: (u64, u64),
    gaps: Vec<i64>,
    in_window: usize,
    months: std::collections::BTreeMap<String, usize>,
    last: String,
}

/// Le bilan de `from` (vide : depuis la première dose) à `to`.
pub fn stats(doses: &[Dose], from: &str, to: &str) -> Stats {
    use std::collections::{BTreeMap, HashMap};
    let mut s = Stats {
        from: from.into(),
        to: to.into(),
        ..Stats::default()
    };
    let people = patients(doses);
    // La DDN connue d'un patient vaut pour toutes ses doses ; sa 1re dose
    // donne le délai de sa 2e, quel que soit l'opérateur.
    let mut births: HashMap<String, String> = HashMap::new();
    let mut firsts: HashMap<String, String> = HashMap::new();
    for p in &people {
        let k = fold(&p.name);
        if !p.birth.is_empty() {
            births.entry(k.clone()).or_insert_with(|| p.birth.clone());
        }
        if let Some(d) = p.d1.first() {
            firsts.entry(k).or_insert_with(|| d.clone());
        }
    }
    let in_period = |day: &str| day >= from && day <= to;
    let mut months: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut team: BTreeMap<String, OpAcc> = BTreeMap::new();
    let (mut sum, mut n) = (0u64, 0u64);
    for d in doses.iter().filter(|d| in_period(&d.date)) {
        let month = month_key(&d.date);
        let m = months.entry(month.clone()).or_default();
        let key = fold(&d.name);
        let who = operator_key(&d.operator).unwrap_or_else(|| "—".into());
        let op = team.entry(who).or_default();
        if d.rank == 2 {
            s.d2 += 1;
            m.1 += 1;
            op.d2 += 1;
            if let Some(first) = firsts.get(&key).filter(|_| !key.is_empty()) {
                if let Some(g) = days_between(first, &d.date).filter(|g| *g >= 0) {
                    op.gaps.push(g);
                    if d.date.as_str() >= add_months(first, D2_FROM_MONTHS).as_str()
                        && d.date.as_str() <= add_months(first, D2_TO_MONTHS).as_str()
                    {
                        op.in_window += 1;
                    }
                }
            }
        } else {
            s.d1 += 1;
            m.0 += 1;
            op.d1 += 1;
        }
        match d.prescriber {
            Prescriber::Officine => {
                s.by_pharmacy += 1;
                op.pharmacy += 1;
            }
            Prescriber::Medecin => s.by_doctor += 1,
            Prescriber::Inconnu => {}
        }
        *op.months.entry(month).or_default() += 1;
        if d.date > op.last {
            op.last = d.date.clone();
        }
        if key.is_empty() {
            s.anonymous += 1;
        } else {
            op.patients.insert(key.clone());
        }
        let known = match births.get(&key) {
            Some(b) if d.birth.is_empty() && !key.is_empty() => age_on(b, &d.date),
            _ => None,
        };
        let band = match known.or_else(|| dose_age(d)) {
            Some(a) => {
                sum += u64::from(a);
                n += 1;
                op.ages.0 += u64::from(a);
                op.ages.1 += 1;
                BANDS
                    .iter()
                    .position(|(lo, hi, _)| a >= *lo && a <= *hi)
                    .unwrap_or(0)
            }
            None => 5,
        };
        s.ages[band] += 1;
    }
    s.mean_age = (n > 0).then(|| sum as f64 / n as f64);
    // Chaque mois, même vide.
    let start = if from.is_empty() {
        months.keys().next().cloned()
    } else {
        Some(month_key(from))
    };
    if let Some(start) = start {
        let mut day = format!("{start}-01");
        let last = month_key(to);
        let mut guard = 0;
        while month_key(&day) <= last && guard < 600 {
            let k = month_key(&day);
            let (a, b) = months.get(&k).copied().unwrap_or_default();
            s.months.push((k, a, b));
            day = add_months(&day, 1);
            guard += 1;
        }
    }
    s.team = team
        .into_iter()
        .map(|(key, a)| OpRow {
            doses: a.d1 + a.d2,
            d1: a.d1,
            d2: a.d2,
            pharmacy: a.pharmacy,
            patients: a.patients.len(),
            mean_age: (a.ages.1 > 0).then(|| a.ages.0 as f64 / a.ages.1 as f64),
            gaps: a.gaps.len(),
            median_gap: median(a.gaps),
            in_window: a.in_window,
            months: s
                .months
                .iter()
                .map(|(m, _, _)| a.months.get(m).copied().unwrap_or(0))
                .collect(),
            last: a.last,
            key,
        })
        .collect();
    s.team
        .sort_by(|a, b| b.doses.cmp(&a.doses).then_with(|| a.key.cmp(&b.key)));
    let mut gaps = Vec::new();
    for p in &people {
        if p.d1.iter().chain(&p.d2).any(|d| in_period(d)) {
            s.people += 1;
        }
        if !p.d1.is_empty() && p.d1[0].as_str() <= to {
            s.started += 1;
            if p.d2.iter().any(|d| d.as_str() <= to) {
                s.completed += 1;
            }
        }
        if let (Some(a), Some(b)) = (p.d1.first(), p.d2.first()) {
            if in_period(b) {
                if let Some(g) = days_between(a, b) {
                    gaps.push(g);
                    if b.as_str() >= add_months(a, D2_FROM_MONTHS).as_str()
                        && b.as_str() <= add_months(a, D2_TO_MONTHS).as_str()
                    {
                        s.in_window += 1;
                    }
                }
            }
        }
    }
    s.gaps = gaps.len();
    s.median_gap = median(gaps);
    s.people += s.anonymous;
    s
}

pub fn pct(v: Option<f64>) -> String {
    v.map(|v| format!("{} %", (v * 100.0).round() as i64))
        .unwrap_or_else(|| "—".into())
}

/// Un délai en jours, dit en mois à la française : « 4,1 mois ».
pub fn months_said(days: Option<i64>) -> String {
    days.map(|g| format!("{:.1} mois", g as f64 / 30.44).replace('.', ","))
        .unwrap_or_else(|| "—".into())
}

// ---------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------

/// Ce qu'un import a fait.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Imported {
    pub added: usize,
    /// Déjà au registre : même jour, même patient.
    pub known: usize,
    /// Lignes illisibles : (numéro, raison).
    pub refused: Vec<(usize, String)>,
    /// Laissées de côté : un autre produit, un retour.
    pub skipped: crate::import::Skipped,
}

/// Lit un CSV dont l'en-tête nomme les colonnes (l'export de ce programme,
/// `date;dose;patient;age;ddn;operateur;prescripteur`, ou un autre que
/// `import::guess` reconnaît) et l'ajoute au registre. L'écran passe par
/// `import::doses`, pour laisser choisir les colonnes.
#[cfg(test)]
pub fn import_csv(store: &mut Store, text: &str, today: &str) -> Imported {
    let table = crate::import::read_table(text);
    let mapping = crate::import::guess(&table.heads);
    let (rows, refused, skipped) = crate::import::doses(&table, &mapping, today);
    let mut out = add_imported(store, rows);
    out.refused = refused;
    out.skipped = skipped;
    out
}

/// Ajoute des doses lues. Une dose de rang 0 se déduit : D2 quand le
/// patient a une D1 sans D2 d'au moins quatre semaines, sinon D1. Une
/// dose déjà au registre (même jour, même patient) n'est pas reprise.
pub fn add_imported(store: &mut Store, mut rows: Vec<(usize, Dose)>) -> Imported {
    let mut out = Imported::default();
    rows.sort_by(|a, b| a.1.date.cmp(&b.1.date).then(a.0.cmp(&b.0)));
    let same = |a: &Dose, b: &Dose| {
        !a.name.trim().is_empty()
            && fold(&a.name) == fold(&b.name)
            && (a.birth.is_empty() || b.birth.is_empty() || a.birth == b.birth)
    };
    for (_, mut d) in rows {
        if store.doses.iter().any(|x| x.date == d.date && same(x, &d)) {
            out.known += 1;
            continue;
        }
        if d.rank == 0 {
            let mut mine: Vec<&Dose> = store.doses.iter().filter(|x| same(x, &d)).collect();
            mine.sort_by(|a, b| a.date.cmp(&b.date));
            let open_d1 = mine
                .iter()
                .rev()
                .find(|x| x.date < d.date)
                .filter(|x| x.rank == 1);
            d.rank = match open_d1 {
                Some(x) if days_between(&x.date, &d.date).is_some_and(|g| g >= 28) => 2,
                _ => 1,
            };
        }
        store.add(d);
        out.added += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dose(date: &str, rank: u8, name: &str, birth: &str) -> Dose {
        Dose {
            id: 0,
            date: date.into(),
            rank,
            name: name.into(),
            age: None,
            birth: birth.into(),
            operator: "ys".into(),
            prescriber: Prescriber::Officine,
        }
    }

    #[test]
    fn the_operator_is_read_whatever_the_case() {
        assert_eq!(operator_key(" ys ").as_deref(), Some("YS"));
        assert_eq!(operator_key("As").as_deref(), Some("AS"));
        assert_eq!(operator_key("é").as_deref(), Some("É"));
        assert_eq!(operator_key(""), None);
        assert_eq!(operator_key("Y S"), None);
        assert_eq!(operator_key("Y1"), None);
        assert_eq!(operator_key("ABCDEF"), None);
    }

    #[test]
    fn dates_read_the_french_way() {
        let t = "2026-11-04";
        assert_eq!(parse_date("03/11/2026", t).unwrap(), "2026-11-03");
        assert_eq!(parse_date("031126", t).unwrap(), "2026-11-03");
        assert_eq!(parse_date("0311", t).unwrap(), "2026-11-03");
        assert_eq!(parse_date("03/04/52", t).unwrap(), "1952-04-03");
        assert_eq!(parse_date("31/02/2026", t), None);
        assert_eq!(parse_date("", t), None);
        assert_eq!(add_months("2026-08-31", 6), "2027-02-28");
        assert_eq!(age_on("1950-11-05", "2026-11-04"), Some(75));
        assert_eq!(month_label("2026-11"), "nov. 26");
    }

    #[test]
    fn doses_group_into_patients_by_name_and_birth() {
        let doses = [
            dose("2026-01-10", 1, "Martin Jeanne", "1950-03-03"),
            dose("2026-03-20", 2, "MARTIN  jeanne", ""),
            dose("2026-02-01", 1, "Martin Jeanne", "1961-01-01"),
            dose("2026-02-01", 1, "Lefèvre Paul", ""),
            dose("2026-02-02", 1, "", ""),
        ];
        let p = patients(&doses);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].d2, vec!["2026-03-20"]);
        assert!(p[1].d2.is_empty());
        assert_eq!(suggest(&p, "lefe", 5)[0].name, "Lefèvre Paul");
        assert_eq!(suggest(&p, "jea", 5).len(), 2);
    }

    #[test]
    fn second_doses_read_soon_now_or_late() {
        let today = "2026-11-04";
        let doses = [
            dose("2026-10-01", 1, "A", ""),
            dose("2026-07-01", 1, "B", ""),
            dose("2026-01-01", 1, "C", ""),
            dose("2026-01-01", 1, "D", ""),
            dose("2026-03-01", 2, "D", ""),
        ];
        let p = pending(&patients(&doses), today);
        let got: Vec<(&str, Due)> = p.iter().map(|x| (x.name.as_str(), x.due)).collect();
        assert_eq!(
            got,
            vec![("C", Due::Late), ("B", Due::Now), ("A", Due::Soon)]
        );
        let by = expected_by_month(&p, today, 3);
        assert_eq!(
            by,
            vec![
                ("2026-11".into(), 2),
                ("2026-12".into(), 1),
                ("2027-01".into(), 0)
            ]
        );
    }

    #[test]
    fn stats_count_the_period_and_the_schemes() {
        let mut a = dose("2026-01-10", 1, "A", "1950-01-01");
        a.age = Some(99); // la DDN l'emporte
        let doses = [
            a,
            dose("2026-04-10", 2, "A", ""),
            dose("2026-05-01", 1, "B", ""),
            dose("2026-05-02", 1, "", ""),
        ];
        let s = stats(&doses, "2026-03-01", "2026-05-31");
        assert_eq!((s.d1, s.d2, s.people, s.anonymous), (2, 1, 3, 1));
        assert_eq!(s.team.len(), 1, "« ys » compté comme « YS »");
        let ys = &s.team[0];
        assert_eq!(
            (ys.key.as_str(), ys.doses, ys.d1, ys.d2, ys.patients),
            ("YS", 3, 2, 1, 2)
        );
        assert_eq!((ys.median_gap, ys.in_window), (Some(90), 1));
        assert_eq!(ys.months, vec![0, 1, 2]);
        assert_eq!(s.months.len(), 3);
        assert_eq!((s.started, s.completed), (2, 1));
        assert_eq!((s.median_gap, s.in_window), (Some(90), 1));
        assert_eq!(s.ages[3], 1, "76 ans par la DDN de sa 1re dose");
        assert_eq!(s.ages[5], 2);
        let all = stats(&doses, "", "2026-05-31");
        assert_eq!(all.months.first().unwrap().0, "2026-01");
    }

    /// Deux opérateurs : chacun ses doses, ses mois, et le délai de la 2e
    /// dose qu'il a faite compté depuis une 1re faite par un autre.
    #[test]
    fn the_team_is_compared_operator_by_operator() {
        let mut doses = vec![
            dose("2026-01-10", 1, "A", ""),
            dose("2026-03-10", 2, "A", ""),
            dose("2026-02-01", 1, "B", ""),
        ];
        doses[1].operator = "as".into();
        doses.push(Dose {
            operator: String::new(),
            ..dose("2026-02-02", 1, "", "")
        });
        let s = stats(&doses, "2026-01-01", "2026-03-31");
        let keys: Vec<(&str, usize)> = s.team.iter().map(|o| (o.key.as_str(), o.doses)).collect();
        assert_eq!(keys, vec![("YS", 2), ("AS", 1), ("—", 1)]);
        let as_ = &s.team[1];
        assert_eq!((as_.d2, as_.median_gap, as_.in_window), (1, Some(59), 1));
        assert_eq!(as_.months, vec![0, 0, 1]);
        assert_eq!(s.team[0].last, "2026-02-01");
    }

    #[test]
    fn the_store_round_trips_and_refuses_a_broken_file() {
        let dir = std::env::temp_dir().join(format!("bilan-shingrix-{}", std::process::id()));
        let path = dir.join("doses.json");
        let mut st = Store::default();
        let a = st.add(dose("2026-11-01", 1, "A", ""));
        st.remove(a);
        let b = st.add(dose("2026-11-01", 2, "A", ""));
        assert!(b > a);
        st.save(&path).unwrap();
        assert_eq!(Store::load(&path).unwrap(), st);
        std::fs::write(&path, "{x").unwrap();
        assert!(Store::load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Le CSV de l'export se relit ; une dose vide se déduit ; un second
    /// import du même fichier n'ajoute rien.
    #[test]
    fn an_import_infers_the_dose_and_skips_what_is_known() {
        let mut s = Store::default();
        s.add(dose("2026-03-02", 1, "Martin Paul", "1950-01-01"));
        let csv = "\u{feff}date;dose;patient;age;ddn;operateur\n\
            02/05/2026;;MARTIN Paul;;01/01/1950;ys\n\
            10/05/2026;;Durand Anne;;;as\n\
            20/05/2026;;Durand Anne;;;as\n\
            12/07/2026;;Durand Anne;;;cl\n\
            12/07/2026;D1;Sans Opérateur;;;\n\
            xx;1;Faux;;;ys\n";
        let r = import_csv(&mut s, csv, "2026-11-04");
        assert_eq!(r.added, 4);
        assert_eq!(r.refused.iter().map(|x| x.0).collect::<Vec<_>>(), [6, 7]);
        let ranks: Vec<(String, u8)> = s.doses.iter().map(|d| (d.date.clone(), d.rank)).collect();
        assert_eq!(
            ranks,
            [
                ("2026-03-02".into(), 1),
                ("2026-05-02".into(), 2),
                ("2026-05-10".into(), 1),
                // Dix jours après la D1 : pas une D2.
                ("2026-05-20".into(), 1),
                ("2026-07-12".into(), 2),
            ]
        );
        assert_eq!(s.doses[1].operator, "YS");
        assert_eq!(s.doses[1].prescriber, Prescriber::Inconnu);
        let again = import_csv(&mut s, csv, "2026-11-04");
        assert_eq!((again.added, again.known), (0, 4));

        // Un en-tête dans un autre ordre, des colonnes en plus.
        let mut s = Store::default();
        let csv = "date;operateur;quantite;prescripteur;patient\n\
            01/09/2026;ng;1;Médecin;A\n\
            02/09/2026;ng;1;off.;B\n\
            03/09/2026;ng;1;voisin;C\n";
        let r = import_csv(&mut s, csv, "2026-11-04");
        assert_eq!((r.added, r.refused.len()), (3, 0));
        // Un prescripteur qui n'est pas l'officine : un médecin.
        assert_eq!(
            s.doses.iter().map(|d| d.prescriber).collect::<Vec<_>>(),
            [
                Prescriber::Medecin,
                Prescriber::Officine,
                Prescriber::Medecin
            ]
        );
        let st = stats(&s.doses, "", "2026-11-04");
        assert_eq!(
            (st.by_pharmacy, st.by_doctor, st.team[0].pharmacy),
            (1, 2, 1)
        );
    }

    /// `BILAN_IMPORT_CHECK=/chemin.csv cargo test import_check -- --nocapture` :
    /// ce qu'un import ferait sur un registre vide, en comptes.
    #[test]
    fn import_check() {
        let Some(path) = std::env::var_os("BILAN_IMPORT_CHECK") else {
            return;
        };
        let text = std::fs::read_to_string(path).unwrap();
        let mut s = Store::default();
        let r = import_csv(&mut s, &text, &today());
        let d2 = s.doses.iter().filter(|d| d.rank == 2).count();
        let people = patients(&s.doses).len();
        println!(
            "ajoutées {} (D1 {}, D2 {d2}), déjà là {}, refusées {:?}, patients {people}",
            r.added,
            r.added - d2,
            r.known,
            r.refused
        );
    }
}
