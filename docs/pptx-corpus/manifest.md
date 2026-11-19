# `.pptx` round-trip corpus: manifest

Ten openly licensed, real-world PowerPoint decks for the external-corpus round-trip test
(`crates/engine/tests/it/pptx_roundtrip.rs`, `an_external_corpus_survives_a_pptx_round_trip`).
The decks are third-party files and are **not** in this repository (`AGENTS.md` §1.1). They are
fetched into a private folder with [`fetch.ps1`](fetch.ps1), which checks each file's SHA-256
against [`manifest.json`](manifest.json). The loss report as it stands today is
[`baseline.md`](baseline.md).

Selected 2026-11-19. Every deck comes from Zenodo and is licensed CC-BY-4.0, which allows
downloading, private modification and testing with attribution. The licence is stated on each
record's landing page, linked in the table. Every file is used as downloaded and unmodified. Each
file's MD5 matched the checksum Zenodo publishes, and a second download from each direct URL gave
the same SHA-256.

## Decks

Authoring app and version come from `docProps/app.xml` (`Application`, `AppVersion`) unless marked
*inferred*. Feature counts come from the package parts (method under [How the decks were checked](#how-the-decks-were-checked)).

| # | File (local name) | Title · author | Landing page · direct URL | Licence (stated at) | Authoring app | Slides · size | Features | SHA-256 · bytes | Round trip today |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `01-premis-tutorial-137-slides.pptx` | *PREMIS Tutorial at iPres2022* · PREMIS Editorial Committee (2022) | [zenodo.org/records/7097121](https://zenodo.org/records/7097121) · [download](https://zenodo.org/records/7097121/files/iPRES2022_PREMIS_Tutorial_20220912.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/7097121)) | Microsoft Office PowerPoint 16.0 (2016 or later) | 137 · 4:3 (9144000×6858000 EMU) | 2 masters, 22 layouts, custom theme "Bambusfletværk"; 605 entrance, 77 emphasis, 118 exit, 7 motion-path effects; fade transitions; 198 groups; 79 cropped pictures; 48 hyperlinks; speaker notes; 6 embedded fonts | `4ecf45aac5be431974e1c498159fe9e04b3d9e06d5a791caf047415c9184ccd1` · 7 895 366 | 1 loss: slide 89, 23 animations became 21 |
| 2 | `02-morph-smartart-nl.pptx` | *Samen beslissen & waardenverheldering met patiënten met beperkte gezondheidsvaardigheden* · Laura Vriese (2026) | [zenodo.org/records/24307200](https://zenodo.org/records/24307200) · [download](https://zenodo.org/records/24307200/files/Presentatie%20LauraVriese%20SDM%20Connect%202026.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/24307200)) | Microsoft Office PowerPoint 16.0 | 14 · 16:9 (12192000×6858000) | 2 masters, 77 layouts, custom theme "24-columns - Cover"; **Morph** on 7 slides; 8 SmartArt diagrams; 40 entrance effects; groups; cropped pictures; hyperlinks; empty notes slides | `82f5cf3de93dc890228882e4e84f9f35f1859c3fa1a535702704f2c740a0ae12` · 9 338 377 | 10 slides differ in pixels (0.96–2.67 %) |
| 3 | `03-japanese-chart-4x3-ppt2010.pptx` | *電子メールによる依頼行動の展開から見る中間言語の発達* · 王冠亮, 史琨茹 (2018) | [zenodo.org/records/5089103](https://zenodo.org/records/5089103) · [download](https://zenodo.org/records/5089103/files/%E7%8E%8B%E5%86%A0%E4%BA%AE%20%E5%92%8C%20%E5%8F%B2%E7%90%A8%E8%8C%B9%20-%202018%20-%20%E9%9B%BB%E5%AD%90%E3%83%A1%E3%83%BC%E3%83%AB%E3%81%AB%E3%82%88%E3%82%8B%E4%BE%9D%E9%A0%BC%E8%A1%8C%E5%8B%95%E3%81%AE%E5%B1%95%E9%96%8B%E3%81%8B%E3%82%89%E8%A6%8B%E3%82%8B%E4%B8%AD%E9%96%93%E8%A8%80%E8%AA%9E%E3%81%AE%E7%99%BA%E9%81%94%E2%80%95%E4%B8%AD%E5%9B%BD%E4%BA%BA%E6%97%A5%E6%9C%AC%E8%AA%9E%E5%AD%A6%E7%BF%92%E8%80%85%E3%81%A8%E6%AF%8D%E8%AA%9E%E8%A9%B1%E8%80%85%E3%81%AE%E7%9B%B8%E9%81%95%E3%82%92%E4%B8%AD%E5%BF%83%E3%81%AB%E2%80%95.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/5089103)) | Microsoft Office PowerPoint 14.0 (**2010**) | 40 · 4:3 | native **chart** (1); 21 tables; Japanese text (≈6 600 CJK characters); speaker notes on 37 slides; 3 entrance effects | `f80ec336c5576ee53be713585e98b0f665a0d12b1d2f2cf370dc38ed615a9d78` · 394 735 | no loss |
| 4 | `04-arabic-morph-motion-paths.pptx` | *The Role of Formal and Informal Communication in Organizing Work in the Organization: A Case Study of Algeria Post* · Selma Yagoub, Hiba Kello (2026) | [zenodo.org/records/23898541](https://zenodo.org/records/23898541) · [download](https://zenodo.org/records/23898541/files/%D8%AF%D9%88%D8%B1%20%D8%A7%D9%84%D8%A7%D8%AA%D8%B5%D8%A7%D9%84%20%D8%A7%D9%84%D8%B1%D8%B3%D9%85%D9%8A%20%D9%88%D8%BA%D9%8A%D8%B1%20%D8%A7%D9%84%D8%B1%D8%B3%D9%85%D9%8A%20%D9%81%D9%8A%20%D8%AA%D9%86%D8%B8%D9%8A%D9%85%20%D8%A7%D9%84%D8%B9%D9%85%D9%84%20%D8%A8%D8%A7%D9%84%D9%85%D9%86%D8%B8%D9%85%D8%A9.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/23898541)) | Microsoft Office PowerPoint 15.0 (**2013**) | 19 · 16:9 | **Arabic** right-to-left text (≈3 200 characters); **Morph** on 16 slides; 179 entrance, 52 emphasis, **341 motion-path** effects; cropped pictures | `94fef8f0aa4457f6b6e2c759c79fd27178599f31df734fb5e438a12e320a8bdf` · 1 737 131 | no loss |
| 5 | `05-video-smartart.pptx` | *That don't impress me much: The effect of the CEO's displayed complex emotions in acquisitions announcement interviews on market response* · Jonas Röttger (2023) | [zenodo.org/records/7985815](https://zenodo.org/records/7985815) · [download](https://zenodo.org/records/7985815/files/233749.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/7985815)) | Microsoft Office PowerPoint 16.0 | 22 · 16:9 | **embedded video** (MP4); 2 SmartArt diagrams; 2 tables; 13 entrance effects; custom theme "5_Basis pagina"; hyperlink | `66a47b17cad4ca9fefc330caff80a64e812a53cc85790fe417248ea0e3a4bf01` · 2 690 138 | 2 slides differ in pixels (2.6 %, 3.4 %) |
| 6 | `06-sections-masters-epfl.pptx` | *Training "Data Organization & Storage Solutions"* · EPFL Library (2026) | [zenodo.org/records/20758495](https://zenodo.org/records/20758495) · [download](https://zenodo.org/records/20758495/files/20260331_Data_Organization_Storage_Solutions.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/20758495), also on slide 2) | Microsoft Office PowerPoint 16.0 | 62 · 16:9 | **7 sections**; **14 masters**, 34 layouts; 84 entrance, 4 exit, 24 motion-path effects; 1 native equation; 15 tables; 56 groups; 73 cropped pictures; 173 hyperlinks; notes | `8668b7ede72486870d1bfe54d94140ed5f8b4ab5200ce924a2fbd5278d7383cf` · 12 242 738 | 8 slides differ in pixels (0.23–1.66 %) |
| 7 | `07-wps-chinese-equations.pptx` | *化核坐标系统与素源模族 2kG±1核元提升的代数、算术及几何统一框架* · Akang (2026) | [zenodo.org/records/24101871](https://zenodo.org/records/24101871) · [download](https://zenodo.org/records/24101871/files/%E5%8C%96%E6%A0%B8%E5%9D%90%E6%A0%87%E7%B3%BB%E7%BB%9F%E4%B8%8E%E7%B4%A0%E6%BA%90%E6%A8%A1%E6%97%8F%202kG%C2%B11%EF%BC%9A%E8%B5%8B%E5%80%BC%E2%80%94%E6%A0%B8%E5%85%83%E2%80%94%E5%87%A0%E4%BD%95%E7%BB%9F%E4%B8%80%E6%A1%86%E6%9E%B6.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/24101871)) | WPS Office presentation ("WPS 演示") 14.0 | 22 · 16:9 | **56 native equations** (`a14:m`); Chinese text (≈3 500 characters); 2 masters; 11 tables; notes | `72894323f3e0b1683130b401cdaaad374c304f5360a6a3fd3e3473d12597eeac` · 467 268 | no loss |
| 8 | `08-google-slides-arabic.pptx` | *شرائح المراجعين المفتوحة في إفريقيا للوحدة التدريبية الثانية* (Open Peer Reviewers in Africa, Module II, Arabic) · PREreview, Aurelia Munene, Daniela Saderi, Johanna Havemann, Johanssen Obanda, Joy Owango and others (2023) | [zenodo.org/records/7509906](https://zenodo.org/records/7509906) · [download](https://zenodo.org/records/7509906/files/Open%20Peer%20Reviewers%20in%20Africa%20-%20Module%20II%20Slide%20deck%20%28ARABIC%29.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/7509906)) | **Google Slides** (*inferred*: shapes named `Google Shape;…`, no `docProps/app.xml`) | 45 · 16:9 (9144000×5143500) | **Arabic** text (≈15 000 characters); **5 embedded fonts**; speaker notes on 44 slides; 6 tables; groups; cropped pictures; 56 hyperlinks; 15 entrance effects | `a04b1f0d87c42003bf68d850711c0c5b3f0147ed02ccc2f4a9e5d22e39687ccb` · 1 852 865 | no loss |
| 9 | `09-keynote-workshop.pptx` | *Literature workflows from search to analysis – Workshop report* · Davnah Urbach, Rainer M Krug, Donat Agosti, Giorgia Camperio, Patrick Ruch, Delphine Clara Zemp (2026) | [zenodo.org/records/20811703](https://zenodo.org/records/20811703) · [download](https://zenodo.org/records/20811703/files/Workshop%20Feedback.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/20811703)) | **Apple Keynote** (*inferred*: empty `app.xml`/`core.xml`, Keynote theme "Dynamic Light", the record also holds the `.key` original) | 10 · 16:9 (24384000×13716000, 1920×1080 pt) | Keynote export structure; 11 entrance, 1 motion-path effect; effect-less transition elements | `f1d239c382b106ab52e8e2026d92da3b1b744430b6cd627a1c733bee05f174ce` · 14 920 469 | no loss |
| 10 | `10-libreoffice-workshop.pptx` | *Workshop "Naming Files and Folders - Best Practices at the Microscope"* · André Lampe, Claudia Giuliani, Harald Binder, Jan Schmoranzer (2026) | [zenodo.org/records/19680929](https://zenodo.org/records/19680929) · [download](https://zenodo.org/records/19680929/files/Workshop%20Naming%20Files%20and%20Folders%20-%20Best%20Practice%20at%20the%20Microscop%20IN-CODE%20v1.0_PPTX.pptx?download=1) | [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) ([record](https://zenodo.org/records/19680929)) | **LibreOffice 25.2.7.2** (Windows) | 24 · 16:9 | LibreOffice export structure (3 masters, 4 layouts); 144 entrance effects; 14 groups; 21 hyperlinks | `36a652b77bca6b4d305825fd9e4d0efa1201ac099c2bc3498c5789f4a2aba8c1` · 3 870 922 | 23 of 24 slides differ in pixels (0.6–5.6 %) |

Total 55 410 009 bytes (53 MB). No deck exceeds 15 MB. The round-trip column is the test as it
stands, which compares SlideCraft's own import with its re-export. What SlideCraft drops when it
*first* opens a deck is in [`baseline.md`](baseline.md).

**Authoring apps:** six decks from Microsoft PowerPoint (2010, 2013, and 16.0 ×4), one each from
Google Slides, Apple Keynote, LibreOffice Impress and WPS Office. No deck was authored or
published by Microsoft, Adobe, Avid or Autodesk. Deck 4's `docProps/core.xml` creator reads
"Compte Microsoft", the default French name of a personal Windows account, not a Microsoft author.

**Personal data:** I read the slide text of all ten. Apart from author names and institutional
contact addresses the authors published on purpose (title slides, contact slides), they hold no
personal data.

## Feature coverage

✓ = present in the package; the number is the count where useful.

| Feature | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|
| Custom masters/layouts, non-default theme | ✓ 2/22 | ✓ 2/77 | | | ✓ | ✓ 14/34 | ✓ 2/12 | ✓ | ✓ | ✓ 3/4 |
| Native charts | | | ✓ 1 | | | | | | | |
| Tables | ✓ | | ✓ 21 | | ✓ | ✓ 15 | ✓ 11 | ✓ 6 | | |
| SmartArt | | ✓ 8 | | | ✓ 2 | | | | | |
| Grouped shapes | ✓ 198 | ✓ | | | | ✓ 56 | | ✓ | | ✓ 14 |
| Cropped pictures | ✓ 79 | ✓ | | ✓ | | ✓ 73 | | ✓ | | |
| Entrance effects | ✓ 605 | ✓ 40 | ✓ 3 | ✓ 179 | ✓ 13 | ✓ 84 | | ✓ 15 | ✓ 11 | ✓ 144 |
| Emphasis effects | ✓ 77 | | | ✓ 52 | | | | | | |
| Exit effects | ✓ 118 | | | | | ✓ 4 | | | | |
| Motion paths | ✓ 7 | | | ✓ 341 | | ✓ 24 | | | ✓ 1 | |
| Transitions (with an effect) | ✓ fade | ✓ morph | | ✓ morph | | | | | | |
| **Morph** | | ✓ 7 | | ✓ 16 | | | | | | |
| Embedded video | | | | | ✓ MP4 | | | | | |
| Embedded audio | | | | | | | | | | |
| Speaker notes (non-empty) | ✓ 22 | | ✓ 37 | | | ✓ 28 | ✓ 7 | ✓ 44 | | |
| Sections | | | | | | ✓ 7 | | | | |
| Hyperlinks | ✓ | ✓ | | | ✓ | ✓ 173 | | ✓ 56 | ✓ | ✓ |
| Equations (`a14:m`) | | | | | | ✓ 1 | ✓ 56 | | | |
| Embedded fonts | ✓ 6 | | | | | | | ✓ 5 | | |
| CJK text | | | ✓ ja | | | | ✓ zh | | | |
| Right-to-left text | | | | ✓ ar | | | | ✓ ar | | |
| 4:3 | ✓ | | ✓ | | | | | | | |
| 16:9 | | ✓ | | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 100+ slides | ✓ 137 | | | | | | | | | |

### Not covered by any openly licensed deck I found

- **Embedded audio.** No CC deck carried an audio part. Video is covered.
- **Hebrew in the ten.** Right-to-left text is covered by two Arabic decks. One Hebrew deck was
  found (alternate A11) but left out: it has 8 slides and no other feature the ten lack.
- **Native PowerPoint equations in quantity.** Deck 6 has a single PowerPoint equation and deck 7
  has 56 from WPS. A PowerPoint deck with 65 equations (alternate A3 below) was left out to make
  room for WPS.
- **Charts beyond one.** Only deck 3 has a native chart. Alternates A5–A7 have more.
- **PowerPoint for Mac and PowerPoint 2007.** Both were found (alternates A6, A7) but left out of
  the ten.

## Alternates (checked, allowed, not chosen)

Kept as substitutes if a deck above disappears. All are CC-BY-4.0 Zenodo records.

| # | Record | Why it is interesting | Why not chosen |
|---|---|---|---|
| A1 | [18644257](https://zenodo.org/records/18644257) | 132 slides, Arabic, embedded video | Overlaps decks 1, 4 and 5 |
| A2 | [5849632](https://zenodo.org/records/5849632) | 355 slides, 563 entrance effects | Few other features; deck 1 covers 100+ slides |
| A3 | [13984443](https://zenodo.org/records/13984443) | 65 native PowerPoint equations | Slot went to WPS (deck 7) for another writer |
| A4 | [7084068](https://zenodo.org/records/7084068) | Equations, Chinese, motion paths | 16.7 MB; decks 3 and 7 cover it |
| A5 | [19351260](https://zenodo.org/records/19351260) | ONLYOFFICE 8.3 writer, native charts | Would push out a required feature |
| A6 | [17694716](https://zenodo.org/records/17694716) | PowerPoint for Mac 16, native charts, notes slides | Same |
| A7 | [24070757](https://zenodo.org/records/24070757) | PowerPoint 12.0 (2007), native charts | Same |
| A8 | [10043580](https://zenodo.org/records/10043580) | Second Keynote export (theme "Basic White") | Deck 9 covers Keynote |
| A9 | [20413048](https://zenodo.org/records/20413048) | PowerPoint for Mac, Morph, SmartArt, 4 masters | Deck 2 covers it |
| A10 | [5089084](https://zenodo.org/records/5089084) | WPS, Chinese, exit and motion-path effects | Deck 7 covers WPS |
| A11 | [19181796](https://zenodo.org/records/19181796) | **Hebrew** text (≈1 800 characters), tables, notes, embedded object | Small (8 slides); Arabic decks 4 and 8 cover right-to-left |

## Rejected candidates

| Candidate | Reason |
|---|---|
| Zenodo records under CC-BY-NC, CC-BY-NC-SA, CC-BY-NC-ND (43 `.pptx` files seen) | NC/ND: not needed, every feature they had is covered by an allowed deck |
| Zenodo records under GPL-3.0 (4 files) and without a licence (4 files) | Not on the allowlist |
| Zenodo [19105757](https://zenodo.org/records/19105757), [19106059](https://zenodo.org/records/19106059) (`fig1.pptx`) | Not Office Open XML: OLE compound files (legacy `.ppt` or encrypted) named `.pptx`. SlideCraft rightly refuses them |
| Zenodo [21262249](https://zenodo.org/records/21262249) | Download was stopped part-way; not evaluated |
| About 400 further Zenodo decks (414 files inspected, 407 readable OOXML; random and targeted samples) | Nothing they covered is missing from the ten and the alternates |
| GitHub code-search hits for `extension:pptx` in MIT, Apache-2.0, CC0 and BSD repositories (177 files in 81 repositories) | A repository's code licence is a weak statement about a slide deck. The files were mostly HTML pages, Git LFS pointers or tiny generated decks |
| figshare search API (LibreOffice, Keynote, Impress queries) | No relevant `.pptx` items |

## Blocked hosts

At the start of the search the environment's network policy refused `zenodo.org`,
`figshare.com`, `api.figshare.com`, `osf.io`, `api.osf.io`, `commons.wikimedia.org` and
`www.nasa.gov` (proxy answered 403). The owner then updated the policy and all of them became
reachable. No host was blocked when the decks were chosen and downloaded, and nothing was routed
around a block.

## How the decks were checked

- Each download went into its own new, empty directory and was never opened with Office or run.
- The packages were read with a stdlib-only Python script (`python3 -I`, `zipfile` and regular
  expressions) kept outside the download directories. Counts come from these parts:
  - `docProps/app.xml` for the authoring app and version;
  - `ppt/presentation.xml` for `sldSz`, `p14:sectionLst` and `p:embeddedFontLst`;
  - `ppt/charts/`, `ppt/diagrams/`, `ppt/media/` and `ppt/notesSlides/`;
  - in each slide: `presetClass` on `p:cTn`, `p:transition` and its effect child, `p159:morph`,
    `a:tbl`, `p:grpSp`, `a:srcRect`, `hlinkClick`, `a14:m`, and the Unicode ranges of `a:t` text.
- SlideCraft opened each deck only through `deckcraft-cli info` and the round-trip test.
