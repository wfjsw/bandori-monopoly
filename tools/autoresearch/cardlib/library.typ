// Edit the Markdown files to update the library content.
// Keep all source files beside this document.
// Compile from this folder: typst compile library.typ library.pdf
// First compilation requires downloading the pinned cmarker package.
#import "@preview/cmarker:0.1.10"

#set document(title: "Character and Card Library")
#set page(paper: "a4", margin: 20mm, numbering: "1")
#set text(font: ("Noto Sans CJK SC", "Microsoft YaHei", "Arial"), size: 10pt)
#set heading(numbering: none)

#align(center)[#text(size: 20pt, weight: "bold")[Character and Card Library]]
#outline(title: [Contents], depth: 1)

// This list controls the order of bands and events in the PDF.
#let sources = (
  "GeneralCards",
  "PoppinParty",
  "Afterglow",
  "PastelPalettes",
  "Roselia",
  "HelloHappyWorld",
  "Morfonica",
  "RaiseASuilen",
  "MyGO",
  "AveMujica",
  "MugendaiMewtype",
  "Sumimi",
  "CRYCHIC",
  "CiRCLESupportTeam",
  "IkkaDumbRock",
  "Millsage",
  "events",
)

#for source in sources {
  pagebreak()
  cmarker.render(
    read(source + ".md"),
    label-prefix: source + "-",
    raw-typst: false,
  )
}
