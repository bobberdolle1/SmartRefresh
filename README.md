# SmartRefresh v2.1

Dynamic refresh rate switching (Software VRR) plugin for Steam Deck OLED via Decky Loader.

Плагин динамического переключения частоты обновления для Steam Deck OLED через Decky Loader.

---

## ⚠️ Important / Важно

**Steam Deck LCD is NOT currently supported.** VRR on LCD has significant hardware limitations that cause flickering and instability. This plugin is designed for **Steam Deck OLED only**.

**Steam Deck LCD в данный момент НЕ поддерживается.** VRR на LCD имеет значительные аппаратные ограничения, вызывающие мерцание и нестабильность. Этот плагин предназначен **только для Steam Deck OLED**.

---

## What it does / Что делает

SmartRefresh automatically adjusts your display refresh rate based on real-time FPS from MangoHud. When your game runs at 45 FPS, the display switches to 45Hz. When performance improves, it scales back up. This saves battery while maintaining smooth visuals.

SmartRefresh автоматически регулирует частоту обновления дисплея на основе FPS в реальном времени от MangoHud. Когда игра работает на 45 FPS, дисплей переключается на 45Hz. Когда производительность улучшается, частота повышается. Это экономит батарею, сохраняя плавность изображения.

## Features / Возможности

- **Real-time FPS monitoring** via MangoHud shared memory
- **Hysteresis algorithm** prevents rapid refresh rate oscillation
- **Three sensitivity presets**: Conservative, Balanced, Aggressive
- **Configurable range**: 45-90Hz for OLED
- **Per-game profiles** with auto-loading
- **Battery tracking** and power savings estimation
- **Suspend/resume handling** with state reset
- **External monitor detection** (auto-pause)

---

- **Мониторинг FPS в реальном времени** через shared memory MangoHud
- **Алгоритм гистерезиса** предотвращает быстрые колебания частоты
- **Три пресета чувствительности**: Консервативный, Сбалансированный, Агрессивный
- **Настраиваемый диапазон**: 45-90Hz для OLED
- **Профили для игр** с автозагрузкой
- **Отслеживание батареи** и оценка экономии энергии
- **Обработка сна/пробуждения** со сбросом состояния
- **Обнаружение внешнего монитора** (автопауза)

## Device Support / Поддержка устройств

| Device / Устройство | Status / Статус | Refresh Range / Диапазон |
|---------------------|-----------------|--------------------------|
| Steam Deck OLED | ✅ Supported | 45-90 Hz |
| Steam Deck LCD | ❌ Not Supported | - |

## Requirements / Требования

- **Steam Deck OLED** with SteamOS
- [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader) installed
- MangoHud enabled (Performance Overlay)

---

- **Steam Deck OLED** с SteamOS
- Установленный [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader)
- Включённый MangoHud (Performance Overlay)

## Installation / Установка

1. Download `SmartRefresh.zip` from [Releases](https://github.com/bobberdolle1/SmartRefresh/releases)
2. Open Decky Loader settings
3. Enable Developer Mode
4. Use "Install Plugin from ZIP"

---

1. Скачайте `SmartRefresh.zip` из [Releases](https://github.com/bobberdolle1/SmartRefresh/releases)
2. Откройте настройки Decky Loader
3. Включите Developer Mode
4. Используйте "Install Plugin from ZIP"

## Usage / Использование

1. Open Quick Access Menu (... button)
2. Go to Decky tab
3. Find SmartRefresh
4. Toggle ON
5. Adjust settings as needed

---

1. Откройте Quick Access Menu (кнопка ...)
2. Перейдите на вкладку Decky
3. Найдите SmartRefresh
4. Включите
5. Настройте по необходимости

## Settings / Настройки

| Setting | Description | Описание |
|---------|-------------|----------|
| Enable | Start/stop control | Запуск/остановка |
| Preset | OLED or Custom | OLED или Custom |
| Refresh Range | Min and max Hz (45-90) | Мин. и макс. Hz (45-90) |
| Sensitivity | Reaction speed | Скорость реакции |
| Adaptive | Auto-adjust by FPS stability | Автоподстройка по стабильности |

### Sensitivity Presets / Пресеты чувствительности

- **Conservative**: 2s drop / 5s increase — most stable
- **Balanced**: 1s drop / 3s increase — default
- **Aggressive**: 500ms drop / 1.5s increase — fastest response

## Troubleshooting / Устранение неполадок

### MangoHud not detected / MangoHud не обнаружен

1. Open Quick Access Menu → Performance
2. Enable Performance Overlay Level (any level)
3. Restart game

### Daemon unreachable / Демон недоступен

1. Reload Decky: Settings → Decky → Reload
2. Restart Steam Deck if needed
3. Check logs: `~/.local/share/smart-refresh/daemon.log`

### Hz Not Changing / Частота не меняется

1. Verify MangoHud is active (FPS counter visible)
2. FPS must be outside ±3 tolerance of current Hz
3. Wait for hysteresis timer (1-5s depending on sensitivity)
4. Check if external display is connected (auto-pauses)

## Building from Source / Сборка из исходников

```bash
# Requires Linux/WSL with Rust and Node.js
./build.sh

# Output: SmartRefresh.zip
```

## Project Structure / Структура проекта

```
SmartRefresh/
├── backend/     # Rust daemon (FPS monitoring, display control)
├── frontend/    # React/TypeScript UI (Decky plugin interface)
├── main.py      # Python plugin wrapper (daemon lifecycle)
└── plugin.json  # Decky manifest
```

## Changelog / История изменений

### v2.1.0
- ❌ Removed LCD support (hardware limitations cause flickering)
- 🔧 Fixed Decky Loader ZIP structure for proper installation
- 📝 Updated documentation

### v2.0.1
- FPS Jitter Tolerance (sticky target)
- Configurable FPS tolerance (2.0-5.0)
- Resume cooldown after wake
- Gamescope frame limiter sync option

### v2.0.0
- Per-game profiles
- Battery tracking
- Adaptive sensitivity
- Metrics dashboard
- Transition log

## License / Лицензия

MIT
