# 0133. Font files under the SIL Open Font License

- Status: Accepted; amends [0006](0006-dependency-policy.md) (a license allowed for font files)
- Date: 2026-10-22

## Context

The engine's UI framework ([ADR-0132](0132-the-engines-ui-framework.md)) draws text with fonts of
its choosing, which the engine must ship:

- the editor and the dev overlay embed the fonts of their interface: one for text, one of fixed
  width, one of icons;
- the framework's tests check shaping, bidirectional order and line breaking in several writing
  systems (Arabic, Hebrew, Devanagari, Thai, Japanese), against fonts held in the repository, so
  that they give the same results on every machine;
- the study's measurements showed that a font found by fallback can cost a hundred times more than
  a font named: the engine names the fonts it draws with.

[ADR-0006](0006-dependency-policy.md) lists the licenses that the engine's dependencies may have:
MIT, Apache-2.0, the BSD licenses, ISC, Zlib, Unicode-3.0, BSL-1.0, CC0-1.0 and MPL-2.0. It was
written for crates. Most fonts that cover many writing systems, and most fonts made for interfaces,
the Noto families, Inter, Source Sans among them, are under the SIL Open Font License 1.1 (OFL): a
license approved by the OSI, written for fonts, which lets them be embedded in and shipped with any
program, free or not, under the program's own license. It asks that the license travel with the
font, that the font not be sold by itself, and that a modified font not take a name that the font's
authors reserved. [ADR-0105](0105-egui-fonts-ship-with-the-dev-overlay.md) already ships egui's
fonts, under the OFL and the Ubuntu Font Licence, by an exception for one crate, which leaves with
egui (ADR-0132).

The owner decided on 2026-10-22, among the options that "Alternatives considered" lists.

## Decision

- **Font files may be under the SIL Open Font License 1.1**, beside the licenses that ADR-0006
  allows. This is for fonts only: code under the OFL stays refused, and a crate that carries a font
  needs an exception of its own in `deny.toml`, as `epaint_default_fonts` has. Licenses that the OSI
  has not approved, the Ubuntu Font Licence among them, stay refused, but for egui's fonts, whose
  exception leaves with egui at Stage 6b ([ADR-0132](0132-the-engines-ui-framework.md)).
- **Each font file in the repository has its license beside it**, and the release of a program that
  embeds or ships one puts its license beside the program, as a release does for egui's fonts.
  [`docs/dependencies.md`](../dependencies.md) lists each font with its source, the programs that
  use it and its license. No tool checks a font's license, as `cargo deny` checks a crate's: the
  change that adds a font records it there, with its license file, and its review checks both.
- **A font cut down to the glyphs a test needs** keeps its name only if its license reserves none;
  otherwise it takes another.
- **Which fonts.** The editor and the dev overlay embed one font for text (Noto Sans unless the
  owner prefers another when the text is first drawn), one of fixed width and one of icons. The
  editor falls back on the system's fonts for the writing systems it does not ship; the overlay,
  which ships in games, does not. The tests use small parts of Noto fonts held in the repository. A
  game ships the fonts it draws with as its assets, and never relies on the system's.
- Each font is fetched at the milestone that first uses it, once the owner agrees.

## Alternatives considered

- **Every writing system embedded in the editor**, with no fallback on the system's fonts: the same
  look on every machine, but tens of megabytes of fonts, the Chinese, Japanese and Korean faces of
  Han differing by language.
- **Only fonts under the licenses that ADR-0006 already allows**: no change of policy, but few fonts
  of quality, and a weak cover of Arabic, Devanagari and Han, for the editor and for the tests.

## Consequences

- ADR-0006's list grows by one license, for one kind of file; `deny.toml`, which checks crates, does
  not change.
- The repository holds fonts and their licenses; each one is recorded in `docs/dependencies.md`.
- A game made with the engine that keeps the editor's or the overlay's fonts ships their licenses
  too.
