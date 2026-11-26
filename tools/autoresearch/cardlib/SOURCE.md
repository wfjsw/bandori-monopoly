# cardlib — vendored fixture provenance

Deterministic fixture for the autoresearch benchmark harness. Do not edit the
vendored files by hand; re-vendor from the source ref instead.

| field | value |
|---|---|
| source repo | https://github.com/ileuxali/bangdream-monopoly |
| ref | `refs/heads/main` |
| commit | `3dd8e3da1190ea5f13f4d3814c2098a1d7e7de46` |
| commit date | 2026-11-22 17:06:25 -0500 |
| author | Leo Li |
| subject | Record local wording publication |
| source subdir | `Docs/CharacterAndCardLibrary/` |
| vendor scope | all 17 `*.md` + `library.typ`; `library.pdf` skipped |

## Fetch command (exact)

Run from the repo root, without touching any worktree or `origin`:

```sh
git fetch --no-tags git@github.com:ileuxali/bangdream-monopoly.git 3dd8e3da1190ea5f13f4d3814c2098a1d7e7de46:refs/tmp/cardlib
git archive refs/tmp/cardlib Docs/CharacterAndCardLibrary | tar -x -C .claude/worktrees/cardlib-upstream
cp .claude/worktrees/cardlib-upstream/Docs/CharacterAndCardLibrary/*.md \
   .claude/worktrees/cardlib-upstream/Docs/CharacterAndCardLibrary/library.typ \
   tools/autoresearch/cardlib/
```

## Verbatim statement

Every file in this directory except `SOURCE.md` and `../cardlib.sha256` is a
byte-for-byte copy of the path named above at the pinned commit — no edits, no
renames, no reformatting, no line-ending normalization.

## SHA-256 of the vendored content

```
d98c4a024c5549d57439f72c8b852a6f7a6ee0571e9fc9328ed2dcc15b4eacc2  cardlib/Afterglow.md
e318bba762e04891307d295985f68ae12f1b1cf6836bb18dc50f1b19513630bd  cardlib/AveMujica.md
8a61ed6e7060277ad229c3280e8246ce6dd583e113e963dd5f4e7da036e5af61  cardlib/CRYCHIC.md
a484002f8eb67708bf6e804125ddafaf58b619f30461546f8cb17152df80872d  cardlib/CiRCLESupportTeam.md
6f983792134545afcac5b9d552a4fc7e4da9e71ea45eaa1774d291feb2f7d8e2  cardlib/GeneralCards.md
9b41d5c3e97722eec98d64c4d6bf9be8f836d4ff934720ee96c4b515b0dc44d3  cardlib/HelloHappyWorld.md
4f1a124a4b7b945999b4f03fcedff59c7309084a66143f473062ce16d5d5c180  cardlib/IkkaDumbRock.md
4e7277d4fc655030ca440fd228032a9235cccc9ea7071e770186a0ab7d366dc7  cardlib/Millsage.md
b64d2088565abae544034e178c914a047c8500682189564c851e3b3a023f6648  cardlib/Morfonica.md
e444e64812d72c2b75f9e05a5d10816890f4be7a1c41f78bdd047906a89b5300  cardlib/MugendaiMewtype.md
f520357e53ec15ac76591a9691dc4cb5fa725b88385d0f327a05ecd6271472ee  cardlib/MyGO.md
aeee0a7bfa4c5a12ccfaebe15273546e79ef1377819b7e1d365ed05e416e966f  cardlib/PastelPalettes.md
282b943925ca31abba62413d546cde082d0aba24e00440c0cdd52b7b5ef77e00  cardlib/PoppinParty.md
6a386cb2402f9fd7e754e601889372873c70e19ca50240dc664612d7dc2a4b5e  cardlib/RaiseASuilen.md
98bc41cf8e071d87924ce77cea4356d5a62ac948530ec62a1356b11b5602910c  cardlib/Roselia.md
fdd0005af62ae86c3cc4dae44aa4c6a7a4b1e2aa381aa9f6a66d10e6c0051287  cardlib/Sumimi.md
ad651b00a22156a3e68c31d48f14bbbd1150802c422eaa0a909056762657d663  cardlib/events.md
764fabaf9c976a5109737da838ad4c317825580e2a86410d6fcec1f92c5cadbf  cardlib/library.typ
```
