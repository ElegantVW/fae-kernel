# Visual identity — Grove / Gleam / Kindling

Proposed names (Gil still names; this is the working set):

| Layer | Name | What it is |
|---|---|---|
| **Mother** | **Grove** | The bigger picture. More than an OS: suite, engines, lore, Kindling, the two houses’ *tools* — the clearing everything grows in. |
| **OS** | **Gleam** | Kernel + userspace as one running light. What you have when Kindling catches and the suite sits on it. |
| **Kernel** | **Kindling** | The fire in the wood. Already struck. |
| **Suite-on-Arch (today)** | faeOS | Fast house. Keeps that name until Gleam boots for real. |

faeOS stays the daily Arch kit. It is not the mother, and it is not the OS we are writing.

Render law: **the identity must survive VGA text, serial ANSI, and a dumb 16-color terminal.** No TTF in the machine. No Imagine PNG as source of truth. Pixel/term art only.

---

## 16 colors (VGA DAC = ANSI index)

Index **13** is **Lilac** — the print color. The first glyph Kindling (and the firmware) draws uses this slot.

VGA DAC is 6-bit (0–63). ANSI is the same index in a 16-color terminal (`\033[95m` for bright magenta = 13).

| # | Name | Hex | RGB | VGA 6-bit | ANSI |
|---|---|---|---|---|---|
| 0 | Night | `#1A1218` | 26,18,24 | 6,4,6 | 30 / bg 40 |
| 1 | Dusk | `#6B6FA8` | 107,111,168 | 26,27,41 | 34 |
| 2 | Moss | `#8FBF9A` | 143,191,154 | 35,47,38 | 32 |
| 3 | Mist | `#8EC4C8` | 142,196,200 | 35,48,49 | 36 |
| 4 | Rose | `#E8A0B4` | 232,160,180 | 57,39,44 | 31 |
| 5 | Violet | `#B08CC8` | 176,140,200 | 43,34,49 | 35 |
| 6 | Honey | `#E8C070` | 232,192,112 | 57,47,28 | 33 |
| 7 | Parchment | `#F0E4EE` | 240,228,238 | 59,56,59 | 37 |
| 8 | Shadow | `#4A3848` | 74,56,72 | 18,14,18 | 90 |
| 9 | Periwinkle | `#A8B4E8` | 168,180,232 | 41,44,57 | 94 |
| 10 | Leaf | `#B8DCB8` | 184,220,184 | 45,54,45 | 92 |
| 11 | Ice | `#C8E8EC` | 200,232,236 | 49,57,58 | 96 |
| 12 | Blush | `#F0B8C8` | 240,184,200 | 59,45,49 | 91 |
| **13** | **Lilac** | **`#D4B4E8`** | **212,180,232** | **52,44,57** | **95** |
| 14 | Gold | `#F0D8A0` | 240,216,160 | 59,53,39 | 93 |
| 15 | Moon | `#FFF8FC` | 255,248,252 | 63,61,62 | 97 |

Soft yellow is **Gold (14)** and **Honey (6)**. Orange sits in Honey. Pinks are Rose / Blush. Purples are Violet / Lilac / Dusk.

**First character:** VGA attribute `0x0D` (lilac on night). Serial: `ESC [ 9 5 m` then the glyph. Never a raw `'c'` before the color is set.

---

## Typography

| Where | Face |
|---|---|
| Kindling / Gleam (the machine) | Hardware VGA 8×16 / 9×16, CP437. Box drawing `+|/-`. No TrueType. |
| Serial / kit TUIs | Same 16 colors. Unicode only when the terminal already has it; firmware never assumes it. |
| Paper / Grove docs | Fraunces (display) + a humanist sans if present; otherwise the term art *is* the logo. |

There is no logo font. The logo is pixels in the 16.

---

## Logo (term / VGA)

Source of truth: [logo.txt](logo.txt). 7×7, CP437-safe (`*`, `/`, `\`, `|`, space). Lilac `*` on Night.

Printed at reset, top-left, before any other glyph.

---

## Do not

- Invent a new V (trademark track is Vanguarda’s).
- Use Cinzel/Fraunces inside the kernel.
- Treat a PNG as the identity.
