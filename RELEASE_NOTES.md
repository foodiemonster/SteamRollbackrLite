# SteamRollbackr v0.6.7

## What's New

### SLSsteam Update Detection & One-Click Update
- **Repo Version Comparison**: Compares your locally installed `slssteam-moon` or `slssteam-moon-64` version against the latest GitHub release (`foodiemonster/slssteam-moon-64` or `swwayps/slsteam-moon`).
- **Clickable `Update available` Badge**: When a newer release is available, **System Status** displays an **`Update available`** badge next to the `slssteam-moon` / `slssteam-moon-64` entry (with a tooltip showing the target version). Clicking it triggers the update directly.

### Fixes
- **Full Steam Unlock Before Clean Reset & Full Uninstall**: Both **Clean Reset & Reinstall (64-bit)** and **Full Uninstall (Vanilla Steam)** now run `unlock_updates` first to restore write permissions on Steam's `package/` directory and remove `Steam.cfg` / `steam.cfg` before sanitizing or updating Steam.
