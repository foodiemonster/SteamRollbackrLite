# SteamRollbackr v0.6.6

## What's New

### Clean 64-bit Reset & Full Uninstall
- **Clean Reset & Reinstall (64-bit)**: Performs a clean-slate 5-phase migration/reinstall to `steamrt64` + `slssteam-moon-64`:
  1. Stops Steam, Lumen, and SLSsteam guardian units.
  2. Restores patched `.desktop` entries and cleans `~/.local/share/SLSsteam/`, `~/.local/state/slsteam-moon/`, `~/.local/share/Lumen/`, and runtime files (while preserving `steamapps/`, `userdata/`, `config/`, and `~/.config/SLSsteam/`).
  3. Unlocks `Steam.cfg` / `steam.cfg` and sanitizes `package/` manifests and `.manifest` overrides.
  4. Runs a clean vanilla Steam update to `steamrt64` using an unhooked launcher (`-forcemanifestsuffix steamrt64 -forcesteamupdate -exitsteam`).
  5. Installs fresh **LuaTools + Lumen** and overlays `slssteam-moon-64` (verifying `ELF64`).
- **Full Uninstall (Vanilla Steam)**: Removes all SLSsteam, Lumen, and LuaTools installations, `.desktop` hooks, and caches, and restores latest vanilla `steamrt64` Steam while preserving games, saves, accounts, and `~/.config/SLSsteam/`.

### Tools Tab & Debug Simulator
- **Reorganized Tools Layout**: Compact 2-column top row for **Clean Reset & Reinstall (64-bit)** and **Full Uninstall (Vanilla Steam)**, followed by unified **SLSsteam Variant** and **LuaTools** switcher cards and **32-bit `libcrypto.so.3`** at the bottom.
- **UI State Simulator (`F12`)**: Press `F12` anywhere to open a non-destructive debug modal that simulates different Steam client, SLSsteam variant, lock, and compatibility states to inspect banners, buttons, and warnings in real time.
