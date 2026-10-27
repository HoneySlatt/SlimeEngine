# The editor's look

The fonts, the style sheet and the themes made for the interfaces of the engine's editor and of its
dev overlay, which the `widgets` example draws in. `slime_ui::look` builds them into a program,
with the feature `editor-look`; its rustdoc says how a program takes them and stacks them, and
[ADR-0148](../../../docs/adr/0148-widgets.md) and
[ADR-0149](../../../docs/adr/0149-the-looks-foundations.md) why they are what they are.

| File | What |
|---|---|
| `Geist.ttf` | Geist, the look's font of text: `Geist[wght].ttf` of Geist's release 1.7.0, the variable font of its upright faces, from weight 100 to 900 |
| `GeistMono.ttf` | Geist Mono, its font of fixed width, for numbers and code: `GeistMono[wght].ttf` of the same release |
| [`Geist.OFL.txt`](Geist.OFL.txt) | The license of both, the SIL Open Font License 1.1 with the fonts' copyright, as `OFL.txt` of Geist's repository at its tag `1.7.0` gives it |
| `Lucide.ttf` | Lucide's font of icons, whole, its 2191 icons: `lucide.ttf` of `lucide-font-1.61.0.zip`, Lucide's release 1.61.0 |
| [`Lucide.codepoints.json`](Lucide.codepoints.json) | The name of each of its icons and its codepoint, from 57400 up, `codepoints.json` of the same archive, which `slime_ui::look::icon` reads |
| [`Lucide.LICENSE.txt`](Lucide.LICENSE.txt) | Lucide's license, ISC, and the MIT license of the icons that it took from Feather, which it lists, as `LICENSE` of Lucide's repository at its tag `1.61.0` gives them |
| [`editor.style`](editor.style) | The look's style sheet: labels, buttons, fields, widgets, menus and their bar, lists to choose from and floating panels styled by their kind and their states, and the classes `window`, `bar`, `panel`, `muted`, `fixed`, `tooltip`, `separator` in a menu, and `axis-x`, `axis-y` and `axis-z` on fields of numbers, naming the themes' roles and scales alone, and Lucide's icons that controls draw by their codepoints |
| [`base.theme`](base.theme) | The scales that every theme shares: spacings on a grid of 4 pixels, radii, sizes of text, the controls' sizes, paddings and durations |
| [`dark.theme`](dark.theme) | The dark theme of colors, the look's own: a palette, then the roles that the style sheet names, computed from it |
| [`light.theme`](light.theme) | The light theme of colors, the same roles |
| [`contrast.theme`](contrast.theme) | The high-contrast theme of colors: black surfaces, white text, every surface bordered |
| [`compact.theme`](compact.theme) | Smaller text, controls and padding, stacked after a theme of colors |

The fonts came from the NixOS package `geist-font` 1.7.0, without a download, unchanged but for
their names; the license, which that package lacks, from
`https://raw.githubusercontent.com/vercel/geist-font/1.7.0/OFL.txt`, with the owner's agreement
(ADR-0133). Their SHA-256:

| File | SHA-256 |
|---|---|
| `Geist.ttf` | `5ae613ae6661671a382974c6f3790426444716913acc3bc93da0ec71c1c51058` |
| `GeistMono.ttf` | `d4fdd6f366b2b7d5a7ba6ca80ba6760ee9774bbc1db79824a5e9324ad3c4c7bb` |

Lucide's files came from its release 1.61.0 on GitHub,
`https://github.com/lucide-icons/lucide/releases/download/1.61.0/lucide-font-1.61.0.zip`
(SHA-256 `70d3fac6cb10e751c74a13623e3a766f2f2d3fce757ddb28ba4d22b959ed59f3`), of which the look
keeps the font and its codepoints, unchanged but for their names, and its license from
`https://raw.githubusercontent.com/lucide-icons/lucide/1.61.0/LICENSE`, with the owner's agreement
([ADR-0150](../../../docs/adr/0150-layers.md)). Their SHA-256:

| File | SHA-256 |
|---|---|
| `Lucide.ttf` | `0b98954b05aba6a770306609847f2d2194395ab225288346c7841a4a50ce2300` |
| `Lucide.codepoints.json` | `4334dba4768d08a38f12caef18aa2818835f130a85f133f069f7ac5b161d6724` |
| `Lucide.LICENSE.txt` | `b495047bd93a9b06913511076f504daba17d5bbeb3e0650f3bb53a4220329c57` |

Geist holds the Latin script, Vietnamese included, and Cyrillic, not Greek: the editor falls back
on the system's fonts for the other writing systems, the overlay, which ships in games, does not
(ADR-0133, [ADR-0139](../../../docs/adr/0139-text-drawn-and-the-display-list.md)). Its license
reserves no name. The italic faces are left out: the look draws none.

A program that builds the look in ships `Geist.OFL.txt` and `Lucide.LICENSE.txt` beside it. The `widgets` example builds it
in, which the Windows artifact of CI and the releases carry, with the license beside it.
