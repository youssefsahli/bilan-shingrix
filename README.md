# Shingrix

Registre des doses de Shingrix et bilan PDF. Rust, egui, Typst embarqué.
Un fichier : `~/.local/share/bilan-shingrix/doses.json` (ou `BILAN_FILE`).

La fenêtre a le visage d'Emacs : un tampon blanc en DejaVu Sans Mono, la
barre d'onglets, la ligne de mode, la zone d'écho. Le PDF a celui d'un
manuel GNU imprimé par Texinfo, en Computer Modern.

Onglets : Saisie · Bilan · Équipe · Rappels. Menu Fichier : Bilan PDF,
Importer CSV, Exporter CSV.

Importer CSV lit le format de l'export
(`date;dose;patient;age;ddn;operateur;prescripteur`), dans l'ordre de
l'en-tête s'il y en a un. Prescripteur : `officine`, `medecin` ou vide.
Une dose vide se déduit : D2 quand le patient a une D1 sans D2 d'au moins
quatre semaines, sinon D1. Une dose déjà au registre (même jour, même
patient) n'est pas reprise.

Saisie : Date · Dose · Patient · Âge · DDN · Opér. (obligatoire, casse
indifférente) · Prescrit (off. : par l'équipe, ce qu'on compte ; méd. :
sur ordonnance d'un médecin). Entrée ajoute, ↓ ↑ choisit un patient connu, C-1 / C-2 la
dose, Échap vide, C-p le PDF, C-TAB l'onglet suivant. Date et opérateur
restent d'une saisie à l'autre. Un clic sur une ligne du registre la
modifie ; Supprimer demande confirmation dans la zone d'écho (o ou n).

Le PDF suit la période choisie : synthèse, doses par mois, âges, D2 à
venir, comparaison de l'équipe. Aucun nom de patient.

Fichier > Modèle du bilan… copie le modèle Typst à côté du registre
(`bilan.typ`) et l'ouvre. En tête, les réglages : titre, nom de
l'officine, parties à garder, texte de fin ; le reste se change aussi.
Le bilan suit ce fichier ; une faute est dite avec sa ligne. Modèle
d'origine le met de côté (`bilan-AAAA-MM-JJ-1.typ`).

    cargo run --release
    cargo test
    BILAN_SAMPLE_PDF=/tmp/bilan.pdf cargo test sample

Captures : `BILAN_TAB=0..3`, `BILAN_SIZE=1024x700`,
`BILAN_SCENE=connus|modifier|supprimer`.

Polices : New Computer Modern (GUST Font License) et DejaVu Sans Mono
(licence Bitstream Vera), `fonts/LICENCES.txt`.

Licence propriétaire, versions gratuites : `LICENSE`. Téléchargements :
la page Releases du dépôt.
