// Bilan Shingrix — modèle Typst, à la manière d'un manuel GNU imprimé par
// Texinfo : Computer Modern, le titre sur un filet épais, des sections
// numérotées, des tableaux sans filets, des figures en noir et gris avec
// leur légende dessous. Les données arrivent en tête, écrites par
// `src/pdf.rs`.
// {{DATA}}

#let fills = (
  luma(35),
  luma(150),
  white,
  luma(210),
  tiling(size: (2.4pt, 2.4pt), line(start: (0pt, 2.4pt), end: (2.4pt, 0pt), stroke: 0.35pt)),
  tiling(size: (2pt, 2pt), place(dx: 0.6pt, dy: 0.6pt, circle(radius: 0.35pt, fill: black))),
  luma(95),
  tiling(size: (2.4pt, 2.4pt), line(start: (0pt, 1.2pt), end: (2.4pt, 1.2pt), stroke: 0.35pt)),
)
#let fill-of(i) = fills.at(calc.rem(i, fills.len()))
#let swatch(f) = box(width: 0.8em, height: 0.8em, baseline: 0.1em, fill: f, stroke: 0.4pt)

#set document(title: "Bilan Shingrix")
#set text(font: "New Computer Modern", size: 10.5pt, lang: "fr")
#set par(justify: true, leading: 0.6em, spacing: 0.9em, first-line-indent: 1.2em)
#set page(
  paper: "a4",
  margin: (x: 27mm, top: 24mm, bottom: 26mm),
  // La ligne de tête de Texinfo : rien sur la première page ; sur les
  // paires, le folio puis le titre ; sur les impaires, la section puis
  // le folio.
  header: context {
    let n = here().page()
    if n > 1 {
      let before = query(selector(heading.where(level: 1)).before(here()))
      let sec = if before.len() > 0 {
        let h = before.last()
        [#numbering("1", ..counter(heading).at(h.location())) #h.body]
      } else { [] }
      if calc.even(n) {
        grid(columns: (auto, 1fr), str(n), align(right)[Bilan Shingrix])
      } else {
        grid(columns: (1fr, auto), sec, str(n))
      }
    }
  },
)

#set heading(numbering: "1.1")
#show heading: set block(above: 1.8em, below: 0.9em)
#show heading.where(level: 1): set text(size: 14.4pt, weight: "bold")
#show heading.where(level: 2): set text(size: 12pt, weight: "bold")

#set table(stroke: none, inset: (x: 0.5em, y: 0.32em))
#set figure(gap: 0.9em, supplement: [Figure])
#show figure: set block(breakable: false, above: 1.4em, below: 1.4em)
#set figure.caption(separator: [ : ])
#show figure.caption: it => align(left, it)

// La page de titre de Texinfo, resserrée : le titre, le filet de 4 pt,
// les sous-titres au fer à droite, l'auteur en gras, le filet de 2 pt.
#block(below: 2.2em)[
  #text(24.9pt, weight: "bold")[Bilan Shingrix]
  #v(-0.9em)
  #line(length: 100%, stroke: 4pt)
  #v(-0.3em)
  #align(right)[
    #text(12pt)[Période du #periode.replace(" – ", " au ")] \
    #text(12pt)[Édité le #edite]
  ]
  #v(0.6em)
  #block(text(12pt, weight: "bold",
    if equipe.len() > 0 [Équipe : #equipe.map(o => o.at(0)).join(", ")] else [Officine]))
  #v(-0.7em)
  #line(length: 100%, stroke: 2pt)
]

// Un diagramme en barres, noir et gris, sur une ligne de base.
#let chart(groups, h: 42mm, shift: 0) = {
  let peak = calc.max(1, ..groups.map(g => calc.max(0, ..g.at(1))))
  let step = if peak <= 5 { 1 } else if peak <= 10 { 2 } else if peak <= 25 { 5 } else if peak <= 50 { 10 } else if peak <= 100 { 20 } else { 50 }
  let ymax = calc.ceil(peak / step) * step
  let n = calc.max(1, groups.len())
  let k = calc.max(1, ..groups.map(g => g.at(1).len()))
  let gutter = if n > 18 { 0.8mm } else { 2.5mm }
  let ticks = range(0, int(ymax / step) + 1)
  set text(8pt)
  block(width: 100%, breakable: false, {
    grid(columns: (8mm, 1fr), column-gutter: 1.5mm,
      box(height: h, {
        for i in ticks {
          place(top + right, dy: h * (1 - i * step / ymax) - 0.6em, str(i * step))
        }
      }),
      box(width: 100%, height: h, {
        for i in ticks {
          place(top + left, dy: h * (1 - i * step / ymax),
            line(length: 100%, stroke: if i == 0 { 0.6pt } else { (paint: luma(190), thickness: 0.3pt, dash: "dotted") }))
        }
        place(bottom + left, grid(columns: (1fr,) * n, column-gutter: gutter, align: bottom,
          ..groups.map(g => {
            let vals = g.at(1)
            grid(columns: (1fr,) * calc.max(1, vals.len()), column-gutter: 0pt, align: bottom,
              ..vals.enumerate().map(((j, v)) => stack(dir: ttb, spacing: 0.8mm,
                align(center, text(6.5pt, if v > 0 and n * k <= 30 { str(v) } else { "" })),
                if v > 0 { rect(width: 100%, height: h * v / ymax, fill: fill-of(j + shift), stroke: 0.4pt) })))
          })))
      }))
    v(0.4em)
    grid(columns: (8mm, 1fr), column-gutter: 1.5mm, [],
      grid(columns: (1fr,) * n, column-gutter: gutter,
        ..groups.map(g => align(center, text(if n > 12 { 6pt } else { 7.5pt }, g.at(0))))))
  })
}

#let mention(f, name) = box[#swatch(f)~#name]

= Synthèse

Ce bilan compte les doses de Shingrix faites à l'officine sur la période,
premières (D1) et deuxièmes (D2). Il ne contient aucun nom de patient :
seulement des comptes, et les initiales de l'équipe.

#pad(left: 1.2em, table(
  columns: (auto, auto, auto),
  align: (left, right, left),
  column-gutter: 1.5em,
  ..tuiles.map(((v, l, s, c)) => (l, strong(v), s)).flatten(),
))

// Le @table de Texinfo : le terme au fer, la définition en retrait.
#let deftable(..items) = grid(
  columns: (22mm, 1fr), row-gutter: 0.65em, column-gutter: 0pt,
  ..items.pos().map(((term, def)) => (term, par(first-line-indent: 0pt, def))).flatten(),
)

#deftable(
  ([Complets], [patients ayant reçu leurs deux doses, parmi ceux qui ont commencé sur la période.]),
  ([Délai], [médiane du temps entre la D1 et la D2 d'un même patient.]),
  ([Fenêtre], [D2 faites entre 2 et 6 mois après la D1, comme le prévoit le schéma.]),
  ([Dues], [D2 qu'on peut faire aujourd'hui ; en retard, celles dont la fenêtre est passée.]),
  ([À venir], [D2 attendues dans les six prochains mois.]),
)

= Doses par mois

#figure(
  chart(mois, h: 36mm),
  caption: [Doses par mois : #mention(fill-of(0), [D1]), #mention(fill-of(1), [D2]).],
)

#block(breakable: false, width: 100%)[
= Âges

L'âge est celui du jour de la dose, tiré de la date de naissance quand
elle est connue.

#pad(left: 1.2em, table(
  columns: (auto, auto, auto, 50mm),
  align: (left, right, right, left + horizon),
  column-gutter: 1em,
  table.header(..([Tranche], [Doses], [Part], []).map(strong)),
  ..ages.map(((l, n)) => (
    l, str(n),
    if total > 0 { str(calc.round(100 * n / total)) + " %" } else { "—" },
    box(width: if total > 0 { 50mm * n / total } else { 0mm }, height: 0.7em, fill: luma(150), stroke: if n > 0 { 0.4pt } else { none }),
  )).flatten(),
))
]

= Deuxièmes doses à venir

#figure(
  chart(attente, h: 30mm, shift: 1),
  caption: [D2 attendues, par mois d'ouverture de leur fenêtre. Le mois en
  cours compte aussi celles qui sont déjà dues.],
)

#if equipe.len() > 0 [
  = Équipe

  Chaque dose porte les initiales de qui l'a faite. Le délai d'une D2 se
  compte depuis la D1 du même patient, quel qu'en soit l'auteur.

  #set text(hyphenate: false)
  #table(
    columns: (1fr,) + (auto,) * 9,
    align: (left, ..range(9).map(_ => right)),
    inset: (x: 0.42em, y: 0.32em),
    table.header(
      ..("Opér.", "Doses", "Part", "D1", "D2", "Patients", "Âge", "Délai", "Fenêtre", "Dernière")
        .map(h => strong(h)),
    ),
    ..equipe.enumerate().map(((i, o)) => (
      [#swatch(fill-of(i))~#o.at(0)],
      ..o.at(1),
    )).flatten(),
  )

  #figure(
    chart(equipe_mois, h: 38mm),
    caption: [Doses par mois et par opérateur :
      #equipe.enumerate().map(((i, o)) => mention(fill-of(i), o.at(0))).join(", ").],
  )
]
