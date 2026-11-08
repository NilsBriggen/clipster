# Fonts

`wine-tahoma.ttf` and `wine-tahoma-bold.ttf` are the Tahoma-compatible fonts made by the [Wine](https://www.winehq.org) project. They are not Microsoft's Tahoma. They come from Wine 11.21 (its `fonts/tahoma.ttf` and `fonts/tahomabd.ttf`) and are licensed under the **GNU Lesser General Public License, version 2.1 or later** (see `LGPL-2.1.txt`). Clipster embeds them to get the classic 90s interface look on every system. They are **modified**: the embedded bitmap strike tables (`EBDT`, `EBLC`, `BDF`) were removed so every size renders from the vector outlines (egui can't colour bitmap strikes). Nothing else was changed.

Copyright (c) the Wine project authors. Source: https://gitlab.winehq.org/wine/wine/-/tree/master/fonts

`NotoSans-Regular.ttf` and `NotoSans-Medium.ttf` are [Noto Sans](https://github.com/notofonts/latin-greek-cyrillic) version 2.015, unmodified, licensed under the **SIL Open Font License 1.1** (see `OFL-1.1.txt`). They are the type of the Desk's Modern look.
