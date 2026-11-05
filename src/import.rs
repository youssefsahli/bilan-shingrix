//! L'import d'un CSV quelconque : un export de LGPI, d'un autre logiciel,
//! ou celui de ce programme. Les colonnes se choisissent (et se devinent
//! par leur en-tête) ; le choix se garde pour le fichier suivant. Pur : pas
//! d'egui, pas d'horloge.

use crate::model::{fold, operator_key, parse_date, Dose, Prescriber};
use serde::{Deserialize, Serialize};

/// Un tableau lu : l'en-tête, puis les lignes (avec leur numéro dans le
/// fichier).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub heads: Vec<String>,
    pub rows: Vec<(usize, Vec<String>)>,
}

/// Des octets en texte : UTF-8, sinon Latin-1 (les exports de tableur et
/// de LGPI le sont souvent).
pub fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Le séparateur le plus fréquent de la première ligne : `;`, tabulation
/// ou `,`.
fn separator(first: &str) -> char {
    [';', '\t', ',']
        .into_iter()
        .max_by_key(|c| first.matches(*c).count())
        .unwrap_or(';')
}

/// Une ligne en cellules ; les guillemets protègent le séparateur, `""`
/// est un guillemet.
fn split(line: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut cell).trim().to_owned()),
            c => cell.push(c),
        }
    }
    out.push(cell.trim().to_owned());
    out
}

pub fn read_table(text: &str) -> Table {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty());
    let Some((_, first)) = lines.next() else {
        return Table::default();
    };
    let sep = separator(first);
    Table {
        heads: split(first, sep),
        rows: lines.map(|(i, l)| (i + 1, split(l, sep))).collect(),
    }
}

/// Le choix des colonnes (par leur en-tête ; vide : aucune) et les
/// réglages de lecture.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    pub date: String,
    pub dose: String,
    pub name: String,
    pub first_name: String,
    pub birth: String,
    pub age: String,
    pub operator: String,
    pub prescriber: String,
    pub qty: String,
    pub product: String,
    /// Une ligne n'est prise que si son produit contient ce mot.
    pub product_filter: String,
    /// Les initiales quand la colonne n'en donne pas.
    pub operator_fixed: String,
    /// Les codes vendeur en initiales : `03=YS 05=AS`.
    pub operator_codes: String,
    /// Ce qui, dans la colonne prescripteur, désigne l'officine (mots
    /// séparés par des espaces ou des virgules). Une autre valeur : un
    /// médecin ; vide : inconnu.
    pub officine_words: String,
}

/// Les colonnes à choisir : (mot de l'écran, obligatoire).
pub const COLUMNS: [(&str, bool); 10] = [
    ("Date", true),
    ("Patient (nom)", false),
    ("Prénom", false),
    ("DDN", false),
    ("Âge", false),
    ("Dose", false),
    ("Opérateur", false),
    ("Prescripteur", false),
    ("Quantité", false),
    ("Produit", false),
];

impl Mapping {
    pub fn column_mut(&mut self, i: usize) -> &mut String {
        match i {
            0 => &mut self.date,
            1 => &mut self.name,
            2 => &mut self.first_name,
            3 => &mut self.birth,
            4 => &mut self.age,
            5 => &mut self.dose,
            6 => &mut self.operator,
            7 => &mut self.prescriber,
            8 => &mut self.qty,
            _ => &mut self.product,
        }
    }

    fn columns(&self) -> [&String; 10] {
        [
            &self.date,
            &self.name,
            &self.first_name,
            &self.birth,
            &self.age,
            &self.dose,
            &self.operator,
            &self.prescriber,
            &self.qty,
            &self.product,
        ]
    }

    /// Chaque colonne choisie existe dans ces en-têtes.
    pub fn fits(&self, heads: &[String]) -> bool {
        !self.date.is_empty()
            && self
                .columns()
                .iter()
                .all(|c| c.is_empty() || heads.iter().any(|h| h == *c))
    }
}

/// Le choix deviné par les en-têtes : « Date de vente », « Nom du
/// patient », « Vendeur », « Qté », « Désignation »…
pub fn guess(heads: &[String]) -> Mapping {
    let folded: Vec<String> = heads.iter().map(|h| fold(h)).collect();
    let find = |want: &[&str], not: &[&str]| -> String {
        for w in want {
            if let Some(i) = folded
                .iter()
                .position(|h| h.contains(w) && !not.iter().any(|n| h.contains(n)))
            {
                return heads[i].clone();
            }
        }
        String::new()
    };
    let birth = find(&["ddn", "naissance", "date nais", "ne le"], &[]);
    let mut m = Mapping {
        date: find(&["date"], &["naissance", "ddn", "nais"]),
        birth,
        first_name: find(&["prenom"], &[]),
        age: find(&["age"], &["usage", "dosage"]),
        dose: find(&["dose", "rang", "injection"], &[]),
        operator: find(
            &[
                "operateur",
                "vendeur",
                "opérateur",
                "operat",
                "initiales",
                "pharmacien",
                "utilisateur",
            ],
            &["prescri"],
        ),
        prescriber: find(&["prescri", "medecin"], &[]),
        qty: find(&["quantite", "qte", "qty", "nombre"], &[]),
        product: find(
            &["designation", "produit", "libelle", "article", "medicament"],
            &[],
        ),
        product_filter: "shingrix".into(),
        officine_words: "officine off pharmacien pharmacie".into(),
        ..Mapping::default()
    };
    m.name = find(
        &["patient", "nom", "client", "beneficiaire", "assure"],
        &["prenom", "produit"],
    );
    if m.name == m.prescriber || m.name == m.product {
        m.name.clear();
    }
    m
}

/// Ce qu'une lecture a laissé de côté, en plus des refus.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skipped {
    /// Un autre produit que celui cherché.
    pub other: usize,
    /// Une quantité nulle ou négative : un retour, une annulation.
    pub returns: usize,
}

/// La date d'une cellule : `05/11/2026 14:32`, `2026-11-05T14:32:00`…
fn day_of(cell: &str, today: &str) -> Option<String> {
    let first = cell.split_whitespace().next().unwrap_or("");
    let first = first.split('T').next().unwrap_or("");
    parse_date(first, today)
}

fn number(cell: &str) -> Option<f64> {
    cell.trim().replace(',', ".").replace(' ', "").parse().ok()
}

/// Les doses lues (rang 0 : à déduire), les refus, ce qui est laissé de
/// côté.
pub type Lines = (Vec<(usize, Dose)>, Vec<(usize, String)>, Skipped);

/// Les doses que donnent les lignes (rang 0 : à déduire), les refus
/// (numéro de ligne, raison), ce qui est laissé de côté.
pub fn doses(table: &Table, m: &Mapping, today: &str) -> Lines {
    let at = |row: &[String], col: &str| -> String {
        if col.is_empty() {
            return String::new();
        }
        table
            .heads
            .iter()
            .position(|h| h == col)
            .and_then(|i| row.get(i))
            .cloned()
            .unwrap_or_default()
    };
    let codes: Vec<(String, String)> = m
        .operator_codes
        .split([' ', ',', ';'])
        .filter_map(|p| p.split_once('='))
        .map(|(a, b)| (fold(a), b.trim().to_owned()))
        .collect();
    let officine: Vec<String> = m
        .officine_words
        .split([' ', ',', ';'])
        .map(fold)
        .filter(|w| !w.is_empty())
        .collect();
    let filter = fold(&m.product_filter);
    let (mut out, mut refused, mut skipped) = (Vec::new(), Vec::new(), Skipped::default());
    for (n, row) in &table.rows {
        let n = *n;
        if !filter.is_empty()
            && !m.product.is_empty()
            && !fold(&at(row, &m.product)).contains(&filter)
        {
            skipped.other += 1;
            continue;
        }
        if let Some(q) = number(&at(row, &m.qty)) {
            if q <= 0.0 {
                skipped.returns += 1;
                continue;
            }
        }
        let Some(date) = day_of(&at(row, &m.date), today) else {
            refused.push((n, "date illisible".into()));
            continue;
        };
        let rank = match fold(&at(row, &m.dose)).trim_start_matches('d') {
            "1" | "1re" | "1ere" | "premiere" => 1,
            "2" | "2e" | "2eme" | "deuxieme" | "rappel" => 2,
            _ => 0,
        };
        let birth = match at(row, &m.birth) {
            b if b.trim().is_empty() => String::new(),
            b => match day_of(&b, today) {
                Some(b) if b <= date => b,
                _ => {
                    refused.push((n, "DDN illisible".into()));
                    continue;
                }
            },
        };
        let typed = at(row, &m.operator);
        let typed = codes
            .iter()
            .find(|(code, _)| *code == fold(&typed))
            .map_or(typed, |(_, ini)| ini.clone());
        let Some(operator) = operator_key(&typed).or_else(|| operator_key(&m.operator_fixed))
        else {
            refused.push((n, "opérateur manquant".into()));
            continue;
        };
        let who = fold(&at(row, &m.prescriber));
        let prescriber = if who.is_empty() {
            Prescriber::Inconnu
        } else if let Some(p) = Prescriber::read(&who).filter(|p| *p != Prescriber::Inconnu) {
            // Les mots de l'export de ce programme.
            p
        } else if officine
            .iter()
            .any(|w| who.split(' ').any(|x| x == w) || who == *w)
        {
            Prescriber::Officine
        } else {
            Prescriber::Medecin
        };
        let name = [at(row, &m.name), at(row, &m.first_name)]
            .iter()
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        out.push((
            n,
            Dose {
                id: 0,
                date,
                rank,
                name,
                age: number(&at(row, &m.age))
                    .map(|a| a as i64)
                    .filter(|a| (18..=115).contains(a))
                    .map(|a| a as u8),
                birth,
                operator,
                prescriber,
            },
        ));
    }
    (out, refused, skipped)
}

/// Un export à la manière de LGPI, pour la scène `import` et les tests :
/// tabulations, heure dans la date, codes vendeur, prescripteur nommé, un
/// autre produit, un retour, un vendeur inconnu, une date illisible.
pub const SAMPLE: &str = "Date de vente\tN° facture\tNom patient\tPrénom\tDate naissance\tVendeur\tPrescripteur\tDésignation\tQté\n\
    02/09/2026 10:12\t1\tMARTIN\tPaul\t01/01/1950\t03\tDR DURAND\tSHINGRIX PDR+SUSP IM FL+FL 1\t1\n\
    03/09/2026 11:00\t2\tBONNET\tAnne\t\t05\tPHARMACIEN\tSHINGRIX PDR+SUSP IM FL+FL 1\t1\n\
    03/09/2026 11:05\t3\tBONNET\tAnne\t\t05\t\tDOLIPRANE 1000MG CPR 8\t2\n\
    04/09/2026 09:00\t4\tBONNET\tAnne\t\t05\tPHARMACIEN\tSHINGRIX PDR+SUSP IM FL+FL 1\t-1\n\
    05/09/2026 09:00\t5\tROUX\tLuc\t\t99\t\tSHINGRIX PDR+SUSP IM FL+FL 1\t1\n\
    jamais\t6\tX\t\t\t03\t\tSHINGRIX\t1\n";

#[cfg(test)]
mod tests {
    use super::*;

    /// Un export à la manière de LGPI : tabulations, Latin-1, heure dans la
    /// date, codes vendeur, prescripteur nommé, retours, autres produits.
    #[test]
    fn an_lgpi_like_export_is_guessed_and_read() {
        let text = SAMPLE;
        let latin1: Vec<u8> = text.chars().map(|c| c as u32 as u8).collect();
        let t = read_table(&decode(&latin1));
        assert_eq!(t.heads.len(), 9);
        let mut m = guess(&t.heads);
        assert_eq!(
            (
                m.date.as_str(),
                m.name.as_str(),
                m.first_name.as_str(),
                m.birth.as_str()
            ),
            ("Date de vente", "Nom patient", "Prénom", "Date naissance")
        );
        assert_eq!(
            (
                m.operator.as_str(),
                m.prescriber.as_str(),
                m.product.as_str(),
                m.qty.as_str()
            ),
            ("Vendeur", "Prescripteur", "Désignation", "Qté")
        );
        assert!(m.fits(&t.heads));
        m.operator_codes = "03=YS 05=as".into();
        let (d, refused, skipped) = doses(&t, &m, "2026-11-05");
        assert_eq!(
            skipped,
            Skipped {
                other: 1,
                returns: 1
            }
        );
        assert_eq!(refused.iter().map(|r| r.0).collect::<Vec<_>>(), [6, 7]);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].1.name, "MARTIN Paul");
        assert_eq!(d[0].1.date, "2026-09-02");
        assert_eq!(d[0].1.birth, "1950-01-01");
        assert_eq!(d[0].1.operator, "YS");
        assert_eq!(d[0].1.prescriber, Prescriber::Medecin);
        assert_eq!(
            (d[1].1.operator.as_str(), d[1].1.prescriber),
            ("AS", Prescriber::Officine)
        );
        // Un opérateur fixe pour les codes inconnus.
        m.operator_fixed = "ng".into();
        assert_eq!(doses(&t, &m, "2026-11-05").0.len(), 3);
    }

    #[test]
    fn quotes_and_commas_are_read() {
        let t = read_table("date,patient,operateur\n\"01/09/2026\",\"Martin, Paul\",ys\n");
        assert_eq!(t.rows[0].1, ["01/09/2026", "Martin, Paul", "ys"]);
        let t = read_table("a;b\n\"x \"\"y\"\"\";z\n");
        assert_eq!(t.rows[0].1, ["x \"y\"", "z"]);
        assert_eq!(read_table(""), Table::default());
    }

    /// Le fichier de ce programme se devine entier.
    #[test]
    fn our_own_export_is_guessed() {
        let heads: Vec<String> = "date;dose;patient;age;ddn;operateur;prescripteur"
            .split(';')
            .map(String::from)
            .collect();
        let m = guess(&heads);
        assert_eq!(
            m.columns().map(|c| c.as_str()),
            [
                "date",
                "patient",
                "",
                "ddn",
                "age",
                "dose",
                "operateur",
                "prescripteur",
                "",
                ""
            ]
        );
    }
}
