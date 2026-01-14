# SmartRefresh

Dynamic refresh rate switching (Software VRR) plugin for Steam Deck via Decky Loader.

## What it does

SmartRefresh automatically adjusts your display refresh rate based on real-time FPS from MangoHud. When your game runs at 45 FPS, the display switches to 45Hz. When performance improves, it scales back up. This saves battery while maintaining smooth visuals.

## Features

- **Real-time FPS monitoring** via MangoHud shared memory
- **Hysteresis algorithm** prevents rapid refresh rate oscillation
- **Three sensitivity presets**: Conservative, Balanced, Aggressive
- **Configurable range**: 40-90Hz

## Requirements

- Steam Deck with SteamOS
- [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader) installed
- MangoHud enabled (Performance Overlay in Quick Access Menu)

## Installation

1. Download the latest `smart-refresh.zip` from Releases
2. Extract to `~/homebrew/plugins/`
3. Restart Decky Loader or reboot

## Usage

1. Open Quick Access Menu (... button)
2. Go to Decky tab
3. Find SmartRefresh
4. Toggle ON to enable
5. Adjust refresh rate range and sensitivity as needed

## Settings

| Setting | Description |
|---------|-------------|
| Enable | Start/stop dynamic refresh rate control |
| Refresh Range | Min and max Hz (40-90) |
| Sensitivity | How quickly it reacts to FPS changes |

### Sensitivity Presets

- **Conservative**: 2s drop / 5s increase — stable, less reactive
- **Balanced**: 1s drop / 3s increase — default
- **Aggressive**: 500ms drop / 1.5s increase — fast reactions

## Building from Source

### On Linux / Steam Deck / WSL
```bash
# Requires: Rust, pnpm or npm
./build.sh
```
Output: `smart-refresh.zip` ready for deployment.

### On Windows (partial build)
Windows can build the frontend but not the Rust daemon (requires Linux linker).

```powershell
cd frontend
npm install
npm run build
```

To build the full plugin, use WSL or a Linux machine:
```bash
# In WSL or Linux
rustup target add x86_64-unknown-linux-gnu
./build.sh
```

## Project Structure

```
smart-refresh/
├── backend/     # Rust daemon (FPS monitoring, display control)
├── frontend/    # React/TypeScript Decky UI
├── main.py      # Python plugin wrapper
└── plugin.json  # Decky manifest
```

## License

MIT
