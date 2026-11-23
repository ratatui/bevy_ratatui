# Example fallback font subsets

Small renamed subsets sourced from
[`bevy_terminal`](https://github.com/gold-silver-copper/bevy_terminal) so
`examples/windowed_fonts.rs` demonstrates CJK, Color Emoji, and Nerd Font
fallbacks via `FontSource::list` without system font discovery. Original
copyright and license name records are kept; the subsets use their own family
and PostScript names.

| File | Source | License |
| --- | --- | --- |
| `FidelityCJK.otf` | Noto Sans CJK SC Regular at `f8d157532fbfaeda587e826d4cd5b21a49186f7c`: CJK, Hangul and full-width samples | [OFL](Noto-OFL.txt), Copyright 2014–2021 Adobe |
| `FidelityColorEmoji.ttf` | Noto Color Emoji at `8998f5dd683424a73e2314a8c1f1e359c19e8742`: selected emoji, modifiers, flags and joined sequences (CBDT/CBLC) | [OFL](Noto-OFL.txt), Copyright 2022 Google Inc. |
| `FidelityNerdSymbols.ttf` | Ghostty's Nerd Fonts-patched JetBrains Mono at `b40acce58dcf77df52231c3798ea58e924647c89`: `0` and the icons U+2630, U+E0C0, U+E60B, U+F408 | JetBrains Mono [OFL](../jetbrains-mono/OFL.txt); the icon sets are [MIT](NerdFonts-MIT.txt) |
