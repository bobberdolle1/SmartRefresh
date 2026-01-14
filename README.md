# SmartRefresh v2.0

Dynamic refresh rate switching (Software VRR) plugin for Steam Deck via Decky Loader.

Плагин динамического переключения частоты обновления для Steam Deck через Decky Loader.

---

## What it does / Что делает

SmartRefresh automatically adjusts your display refresh rate based on real-time FPS from MangoHud. When your game runs at 45 FPS, the display switches to 45Hz. When performance improves, it scales back up. This saves battery while maintaining smooth visuals.

SmartRefresh автоматически регулирует частоту обновления дисплея на основе FPS в реальном времени от MangoHud. Когда игра работает на 45 FPS, дисплей переключается на 45Hz. Когда производительность улучшается, частота повышается. Это экономит батарею, сохраняя плавность изображения.

## Features / Возможности

- **Real-time FPS monitoring** via MangoHud shared memory
- **Hysteresis algorithm** prevents rapid refresh rate oscillation
- **Three sensitivity presets**: Conservative, Balanced, Aggressive
- **Configurable range**: 40-90Hz (OLED) / 40-60Hz (LCD)
- **LCD Compatibility Mode**: Hardware-specific throttling to prevent screen flickering

---

- **Мониторинг FPS в реальном времени** через shared memory MangoHud
- **Алгоритм гистерезиса** предотвращает быстрые колебания частоты
- **Три пресета чувствительности**: Консервативный, Сбалансированный, Агрессивный
- **Настраиваемый диапазон**: 40-90Hz (OLED) / 40-60Hz (LCD)
- **Режим совместимости с LCD**: аппаратное ограничение для предотвращения мерцания

### New in v2.0 / Новое в v2.0

- **FPS Jitter Tolerance**: "Sticky target" prevents switching when FPS is within ±3 of current Hz
- **Adaptive Sensitivity**: Auto-adjusts based on FPS stability (std dev)
- **Per-Game Profiles**: Save and auto-load settings per game
- **Battery Tracking**: Estimates power savings from dynamic refresh
- **Suspend/Resume Handling**: Resets state on wake to prevent stale timestamps
- **Multi-Monitor Detection**: Auto-pauses when external display connected
- **FPS/Hz Sparkline Graph**: Visual history of last 30 seconds
- **Transition Log**: See recent Hz switches with timestamps
- **Metrics Dashboard**: Switch counts, uptime, stability stats

---

- **Защита от дрожания FPS**: "Липкая цель" предотвращает переключение при FPS в пределах ±3 от текущего Hz
- **Адаптивная чувствительность**: Автоподстройка на основе стабильности FPS
- **Профили для игр**: Сохранение и автозагрузка настроек для каждой игры
- **Отслеживание батареи**: Оценка экономии энергии
- **Обработка сна/пробуждения**: Сброс состояния при выходе из сна
- **Обнаружение внешнего монитора**: Автопауза при подключении
- **График FPS/Hz**: Визуальная история за последние 30 секунд
- **Журнал переключений**: Последние изменения Hz с временными метками
- **Панель метрик**: Счётчики переключений, время работы, статистика

## Device Support / Поддержка устройств

| Device / Устройство | Refresh Range / Диапазон | Min Interval / Мин. интервал | Notes / Примечания |
|---------------------|--------------------------|------------------------------|-------------------|
| Steam Deck OLED | 45-90 Hz | 500ms | Full VRR-like / Полный VRR |
| Steam Deck LCD | 40-60 Hz | 2000ms | Throttled / Ограничено |

## Requirements / Требования

- Steam Deck with SteamOS / Steam Deck с SteamOS
- [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader) installed / установлен
- MangoHud enabled (Performance Overlay) / MangoHud включён

## Installation / Установка

### Quick Install / Быстрая установка

```bash
curl -L https://github.com/bobberdolle1/SmartRefresh/raw/master/install.sh | sh
```

### Manual Install / Ручная установка

1. Download from [Releases](https://github.com/bobberdolle1/SmartRefresh/releases) / Скачайте с Releases
2. Transfer to Steam Deck / Перенесите на Steam Deck
3. Enable Developer Mode in Decky / Включите Developer Mode в Decky
4. Use "Install Plugin from ZIP" / Используйте "Install Plugin from ZIP"

## Usage / Использование

1. Open Quick Access Menu (... button) / Откройте Quick Access Menu
2. Go to Decky tab / Перейдите на вкладку Decky
3. Find SmartRefresh / Найдите SmartRefresh
4. Select device type (OLED/LCD) / Выберите тип устройства
5. Toggle ON / Включите
6. Adjust settings as needed / Настройте по необходимости

## Settings / Настройки

| Setting | Description | Описание |
|---------|-------------|----------|
| Device Preset | OLED, LCD, or Custom | OLED, LCD или Custom |
| Enable | Start/stop control | Запуск/остановка |
| Refresh Range | Min and max Hz | Мин. и макс. Hz |
| Sensitivity | Reaction speed | Скорость реакции |
| Adaptive | Auto-adjust by FPS stability | Автоподстройка по стабильности |

### Sensitivity Presets / Пресеты чувствительности

- **Conservative / Консервативный**: 2s drop / 5s increase — stable / стабильно
- **Balanced / Сбалансированный**: 1s drop / 3s increase — default / по умолчанию
- **Aggressive / Агрессивный**: 500ms drop / 1.5s increase — fast / быстро (OLED only)

## Troubleshooting / Устранение неполадок

### MangoHud not detected / MangoHud не обнаружен

**Solution / Решение**:
1. Open Quick Access Menu → Performance
2. Enable Performance Overlay Level / Включите Performance Overlay
3. Restart game / Перезапустите игру

### Daemon unreachable / Демон недоступен

**Solution / Решение**:
1. Reload Decky: Settings → Decky → Reload
2. Restart Steam Deck if needed / Перезагрузите при необходимости
3. Check logs: `~/.local/share/smart-refresh/daemon.log`

### LCD Flickering / Мерцание на LCD

**Solution / Решение**:
1. Select "Steam Deck LCD" device / Выберите "Steam Deck LCD"
2. Use "LCD Preset (40-60 Hz)" / Используйте пресет LCD
3. Sensitivity will be forced to Conservative / Чувствительность будет консервативной

### Hz Not Changing / Частота не меняется

**Solution / Решение**:
1. Verify MangoHud is active / Проверьте MangoHud
2. FPS must be outside ±3 tolerance / FPS должен выйти за пределы ±3
3. Wait for hysteresis (1-5s) / Подождите гистерезис
4. Check for external display / Проверьте внешний монитор

## Building from Source / Сборка из исходников

```bash
# Linux / Steam Deck / WSL
./build.sh
# Output: smart-refresh.zip
```

Windows can only build frontend / Windows может собрать только frontend:
```powershell
cd frontend
npm install && npm run build
```

## Project Structure / Структура проекта

```
smart-refresh/
├── backend/     # Rust daemon
├── frontend/    # React/TypeScript UI
├── main.py      # Python plugin wrapper
└── plugin.json  # Decky manifest
```

## License / Лицензия

MIT
