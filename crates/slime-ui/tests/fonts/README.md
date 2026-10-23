# The fonts of slime-ui's tests

Parts of Noto fonts, under the SIL Open Font License 1.1
([ADR-0133](../../../../docs/adr/0133-font-files-under-the-open-font-license.md)), that hold the
characters of the texts in [`../texts`](../texts) and what their layout tables reach from them, and
no more: some 700 KiB in all, where the whole fonts take some 45 MiB. slime-ui's tests, the
benchmarks of text (`benches/README.md`) and the `text` example lay text out with them, so that
their results are the same on every machine, whatever fonts it has.

[`scripts/subset-fonts.sh`](../../../../scripts/subset-fonts.sh) makes them again: after a text of
`../texts` changes, so that the fonts hold its characters. It checks that its sources are the
releases below, by their SHA-256, cuts them down with HarfBuzz's `hb-subset` (14.5.1 made these),
drops their hinting, which the engine does not use, keeps every name, and pins every font at its
regular weight, but Noto Sans, which keeps its weight axis so that the tests can draw it bold.

| File | Family | Source | Version | License |
|---|---|---|---|---|
| `NotoSans.ttf` | Noto Sans | `NotoSans.ttf` of the Noto release of 2026-10-01 | 2.015 | [`NotoSans.OFL.txt`](NotoSans.OFL.txt) |
| `NotoSansArabic.ttf` | Noto Sans Arabic | `NotoSansArabic.ttf`, the same release | 2.013 | [`NotoSansArabic.OFL.txt`](NotoSansArabic.OFL.txt) |
| `NotoSansHebrew.ttf` | Noto Sans Hebrew | `NotoSansHebrew.ttf`, the same release | 3.001 | [`NotoSansHebrew.OFL.txt`](NotoSansHebrew.OFL.txt) |
| `NotoSansDevanagari.ttf` | Noto Sans Devanagari | `NotoSansDevanagari.ttf`, the same release | 2.007 | [`NotoSansDevanagari.OFL.txt`](NotoSansDevanagari.OFL.txt) |
| `NotoSansThai.ttf` | Noto Sans Thai | `NotoSansThai.ttf`, the same release | 2.001 | [`NotoSansThai.OFL.txt`](NotoSansThai.OFL.txt) |
| `NotoSansCJKjp.otf` | Noto Sans CJK JP | face 0 of `NotoSansCJK-VF.otf.ttc`, Noto CJK Sans 2.004 | 2.004 | [`NotoSansCJKjp.OFL.txt`](NotoSansCJKjp.OFL.txt) |
| `NotoSansCJKsc.otf` | Noto Sans CJK SC | face 2 of the same collection | 2.004 | [`NotoSansCJKsc.OFL.txt`](NotoSansCJKsc.OFL.txt) |
| `NotoColorEmoji.ttf` | Noto Color Emoji | `NotoColorEmoji.ttf`, Noto Emoji 2.051 | 2.051 | [`NotoColorEmoji.OFL.txt`](NotoColorEmoji.OFL.txt) |

The sources came from the NixOS packages that hold those releases, `noto-fonts` 2026.10.01,
`noto-fonts-cjk-sans` 2.004 and `noto-fonts-color-emoji` 2.051, without a download; the script
gives the SHA-256 of each source file. Every font but the emoji is variable in its release:
hb-subset pins each at weight 400, its regular, but Noto Sans.

Each license file holds the font's copyright, as its `name` table gives it, and the OFL. None of
these licenses reserves the name of the font, but Noto CJK's, which reserves `Source`, a name that
Noto Sans CJK does not take: each part keeps its family's name (ADR-0133).

A program that ships these fonts ships their license files beside them. The tests, the benchmarks
and the examples ship nowhere.
