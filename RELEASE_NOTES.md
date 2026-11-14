# SteamRollbackr v0.6.5

## What's New

### Native 64-bit `slssteam-moon-64` Support
- **Automatic Variant Detection**: Inspects `~/.local/share/SLSsteam/SLSsteam.so` (`ELF32` vs `ELF64`) to automatically detect whether you're running **32-bit `slssteam-moon`** or **64-bit `slssteam-moon-64`**.
- **Variant-Aware Compatibility & Rollback**:
  - `slssteam-moon-64` checks compatibility against [`foodiemonster/slssteam-moon-64`](https://github.com/foodiemonster/slssteam-moon-64) and targets `steamrt64` releases.
  - `slssteam-moon` continues using `swwayps/steam-monitor` and `swwayps/slsteam-moon` for `ubuntu12_32` / legacy builds.
- **One-Click 64-bit Migration**: Users on a rolled-back `ubuntu12` Steam client or 32-bit `slssteam-moon` now see a dismissible callout on **Home** (*"Switch to 64-bit (Unlock & Install)"*) to unlock/update Steam to `steamrt64` and install `slssteam-moon-64` in one step.
- **SLSsteam Variant Switcher in Tools**: Install, reinstall, or switch between `slssteam-moon-64` (64-bit) and `slssteam-moon` (32-bit) directly from **Tools**. Installing or reinstalling LuaTools while `slssteam-moon-64` is active automatically preserves the 64-bit build.

### UI & Polish
- **Context-Aware Compatibility Banner**: When Steam is unlocked and up to date, the green banner now clearly states that everything is working and no rollback or unlock is needed (instead of prompting to Unlock & Update).
- **Client/Variant Mismatch Warnings**: Surfaces a clear warning if Steam is on `steamrt64` with 32-bit `SLSsteam.so` (or vice versa).
- **Sidebar & Icon Updates**: Moved **Tools** above **Advanced** in the sidebar and updated the application icon (`packaging/steamrollbackr.png`).
