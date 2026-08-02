<div align="center">

# vois.rs

Real-time voice changer for your mic: pitch/formant shifting, 12 effect presets and a built-in virtual microphone — written in pure Rust.

![Rust](https://img.shields.io/badge/Rust-1.74+-orange?style=flat-square&logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-blue?style=flat-square)
![PipeWire](https://img.shields.io/badge/PipeWire-supported-purple?style=flat-square)
![License](https://img.shields.io/badge/license-GPL--3.0--or--later-red?style=flat-square&logo=gnu&logoColor=white)
![version](https://img.shields.io/badge/version-0.1.0-green?style=flat-square)

[English](#english) | [Русский](#russian)

</div>

---

<a name="english"></a>

## English

### Overview

vois.rs turns your microphone into a voice changer in real time: shift pitch and formants with a phase-vocoder DSP chain, apply game/voice-change style presets (Robot, 8-bit, Demon, Girl, Alien, Ghost...), and expose the processed voice as a **virtual microphone** that Discord, Zoom, OBS and games can use — no external tools required.

> **Heads up:** this project was hacked together very quickly on a "vibe" and may contain bugs, rough edges and half-finished bits. Use at your own risk and report issues!

### Features

| Feature | Description |
|---------|-------------|
| Real-time pitch shift | Phase-vocoder, −12…+12 semitones, formants preserved |
| Formant shift | Change the timbre (girl / deep / bright) independently of pitch |
| 12 presets | Clean, Girl/Anime, Boy, Manly/Deep, Demon, Robot, 8-bit, Alien, Radio, Megaphone, Ghost, Cyborg |
| Effects | Vocoder, bitcrush, distortion, reverb, chorus, ring-mod, bandpass, noise, compressor, noise gate |
| Virtual microphone | Built-in `vois.rs` mic (PipeWire/PulseAudio) — no VB-CABLE needed on Linux |
| Auto-Tune | Optional pitch snap to the nearest semitone (smooth, no wobble) |
| Config file | Defaults + custom presets in `~/.config/vois/config.toml` |
| TUI | Full-screen terminal UI with arrow-key + mouse control and live meters |
| WAV recording | Record the processed audio with the `R` hotkey or `--record` |
| Test tone | `--tone 220` lets you hear what a preset does without a mic |
| Cross-platform | Linux (ALSA/PipeWire), macOS (CoreAudio), Windows (WASAPI) |

### Presets

| Preset | Pitch | Formant | Effects |
|--------|-------|---------|---------|
| Clean | 0 st | 1.00× | passthrough (A/B compare) |
| Girl / Anime | +5 st | 1.25× | — |
| Boy | +3 st | 1.12× | — |
| Manly / Deep | −4 st | 0.78× | compressor |
| Demon | −9 st | 0.85× | distortion + reverb |
| Robot | 0 st | 1.00× | channel vocoder + bitcrush |
| 8-bit | 0 st | 1.00× | bitcrush (5-bit, decimate ×4, auto-level) |
| Alien | −2 st | 1.15× | ring-mod + chorus |
| Radio | 0 st | 1.00× | bandpass + noise + compressor |
| Megaphone | 0 st | 1.00× | distortion + narrow EQ + compressor |
| Ghost | −3 st | 1.05× | big reverb + chorus |
| Cyborg | +2 st | 1.00× | vocoder + bitcrush |
| Chipmunk | +7 st | 1.40× | bitcrush |
| Helium | +10 st | 1.50× | chorus |
| Vader | −10 st | 0.60× | reverb |
| Telephone | 0 st | 1.00× | bandpass + noise + compressor |
| Growl | −5 st | 0.70× | distortion + reverb |
| Walkie-Talkie | 0 st | 1.05× | narrow bandpass + bitcrush + squelch noise + compressor |

### Usage

```bash
make install     # installs `vois` to ~/.local/bin (make sure it's on PATH)
vois             # just works: auto-picks devices, virtual mic on, 48 kHz
vois -p "Girl / Anime"
vois --no-virtual-mic            # output to your speakers instead
vois --pitch-correct             # Auto-Tune on
vois --tone 220                  # hear presets without a mic
vois --list / --list-presets     # list devices / presets
```

In Discord / Zoom / OBS select **`vois.rs`** as your microphone.

### TUI controls

| Key / Mouse | Action |
|-------------|--------|
| `↑` / `↓` / click | switch preset (live preview) |
| `←` / `→` | change pitch (main) / change setting (settings) |
| `S` / `Tab` | open settings screen |
| `H` | help |
| `Q` / `Ctrl+C` | quit |

Settings screen: `↑↓` select, `←→` / `Enter` change, `Esc` back — preset, pitch, formant, gate threshold (dB), gain (dB), mute, LIVE/PASSTHROUGH mode, record, Auto-Tune (pitch corr).

### Config file

Create `~/.config/vois/config.toml` to set defaults and define your own presets:

```toml
# ~/.config/vois/config.toml
preset = "Deep Robot"
virtual_mic = true

[custom_presets]
"Deep Robot" = { pitch = -6, formant = 0.8, effects = [
    { Vocoder = { bands = 16, carrier = "Noise", wet = 1.0 } },
    { Reverb = { room = 0.5, damp = 0.4, wet = 0.3 } },
] }
```

Any CLI option overrides the file. Effects: `Distortion`, `Bitcrush`, `Reverb`,
`Chorus`, `RingMod`, `Bandpass`, `Noise`, `Compressor`, `Vocoder` (carrier:
`Noise` / `Saw`).

### Dependencies

- `cpal` — audio capture/playback (ALSA / WASAPI / CoreAudio)
- `rustfft` — phase-vocoder FFT
- `ratatui` + `crossterm` — terminal UI
- `ringbuf` — lock-free sample buffers
- `hound` — WAV recording
- Linux virtual mic requires `pactl` (PipeWire/PulseAudio)

### Installation

1. `cargo build --release`
2. Run `./target/release/vois`
3. Select **`vois.rs`** as your microphone in your voice app

### Roadmap / ideas

- Windows/macOS virtual mic (VB-CABLE / BlackHole) as output device
- Latency meter and lower-latency FFT modes
- Pitch auto-correction / karaoke-style smoothing
- More presets and user-defined chains
- Web UI / tray icon
- Playback of a soundboard through the virtual mic

---

<a name="russian"></a>

## Русский

### Обзор

vois.rs превращает твой микрофон в войс-ченджер в реальном времени: сдвиг высоты и формант на фазовом вокодере, 12 пресетов в стиле войс-модов (Robot, 8-bit, Demon, Girl, Alien, Ghost...) и **виртуальный микрофон**, который видят Discord, Zoom, OBS и игры — без внешних программ.

> **Важно:** проект был написан очень быстро, «на вайбе», поэтому могут быть баги, недоделки и шероховатости. Пользуйся на свой страх и риск, баги репорть!

### Возможности

| Возможность | Описание |
|-------------|----------|
| Сдвиг высоты | Фазовый вокодер, −12…+12 полутонов, форманты сохраняются |
| Сдвиг формант | Меняет тембр (девушка / глубокий / звонкий) независимо от высоты |
| 12 пресетов | Clean, Girl/Anime, Boy, Manly/Deep, Demon, Robot, 8-bit, Alien, Radio, Megaphone, Ghost, Cyborg |
| Эффекты | Вокодер, биткраш, дисторшн, реверб, хорус, кольцевая модуляция, полосовой фильтр, шум, компрессор, шумоподавитель |
| Виртуальный микрофон | Встроенный `vois.rs` (PipeWire/PulseAudio) — на Linux не нужен VB-CABLE |
| Автолюн (Auto-Tune) | Снап высоты к ближайшему полутону (плавно, без «плавания») |
| Конфиг-файл | Дефолты и свои пресеты в `~/.config/vois/config.toml` |
| TUI | Полноэкранный интерфейс с управлением стрелками и мышью, живые уровни |
| Запись в WAV | Клавиша `R` или `--record` |
| Тестовый тон | `--tone 220` — послушать пресет без микрофона |
| Кроссплатформенность | Linux (ALSA/PipeWire), macOS (CoreAudio), Windows (WASAPI) |

### Пресеты

| Пресет | Высота | Форманты | Эффекты |
|--------|--------|----------|---------|
| Clean | 0 пт | 1.00× | passthrough (сравнение A/B) |
| Girl / Anime | +5 пт | 1.25× | — |
| Boy | +3 пт | 1.12× | — |
| Manly / Deep | −4 пт | 0.78× | компрессор |
| Demon | −9 пт | 0.85× | дисторшн + реверб |
| Robot | 0 пт | 1.00× | вокодер + биткраш |
| 8-bit | 0 пт | 1.00× | биткраш (5 бит, децимация ×4, автолевел) |
| Alien | −2 пт | 1.15× | кольцевая модуляция + хорус |
| Radio | 0 пт | 1.00× | полосовой фильтр + шум + компрессор |
| Megaphone | 0 пт | 1.00× | дисторшн + узкий EQ + компрессор |
| Ghost | −3 пт | 1.05× | большой реверб + хорус |
| Cyborg | +2 пт | 1.00× | вокодер + биткраш |
| Chipmunk | +7 пт | 1.40× | биткраш |
| Helium | +10 пт | 1.50× | хорус |
| Vader | −10 пт | 0.60× | реверб |
| Telephone | 0 пт | 1.00× | полосовой фильтр + шум + компрессор |
| Growl | −5 пт | 0.70× | дисторшн + реверб |
| Walkie-Talkie | 0 пт | 1.05× | узкий bandpass + биткраш + сквилч-шум + компрессор |

### Запуск

```bash
make install     # ставит `vois` в ~/.local/bin (проверь что он в PATH)
vois             # просто работает: авто-устройства, виртуальный микрофон, 48 кГц
vois -p "Girl / Anime"
vois --no-virtual-mic            # вывод в колонки/наушники
vois --pitch-correct             # автолюн
vois --tone 220                  # послушать пресеты без микрофона
vois --list / --list-presets     # список устройств / пресетов
```

В Discord / Zoom / OBS выбери микрофон **`vois.rs`**.

### Управление в TUI

| Клавиша / мышь | Действие |
|----------------|----------|
| `↑` / `↓` / клик | переключение пресета (живой предпросмотр) |
| `←` / `→` | pitch (главный экран) / изменение параметра (настройки) |
| `S` / `Tab` | экран настроек |
| `H` | справка |
| `Q` / `Ctrl+C` | выход |

Экран настроек: `↑↓` выбор, `←→` / `Enter` изменение, `Esc` назад — пресет, pitch, formant, порог шумоподавителя (дБ), усиление (дБ), mute, режим LIVE/PASSTHROUGH, запись, автолюн.

### Конфиг-файл

Создай `~/.config/vois/config.toml` для дефолтов и своих пресетов:

```toml
# ~/.config/vois/config.toml
preset = "Deep Robot"
virtual_mic = true

[custom_presets]
"Deep Robot" = { pitch = -6, formant = 0.8, effects = [
    { Vocoder = { bands = 16, carrier = "Noise", wet = 1.0 } },
    { Reverb = { room = 0.5, damp = 0.4, wet = 0.3 } },
] }
```

Любой CLI-флаг перекрывает файл. Эффекты: `Distortion`, `Bitcrush`, `Reverb`,
`Chorus`, `RingMod`, `Bandpass`, `Noise`, `Compressor`, `Vocoder` (carrier:
`Noise` / `Saw`).

### Зависимости

- `cpal` — захват/вывод звука (ALSA / WASAPI / CoreAudio)
- `rustfft` — FFT для фазового вокодера
- `ratatui` + `crossterm` — терминальный интерфейс
- `ringbuf` — lock-free буферы сэмплов
- `hound` — запись WAV
- Виртуальный микрофон на Linux требует `pactl` (PipeWire/PulseAudio)

### Установка

1. `cargo build --release`
2. Запусти `./target/release/vois`
3. В голосовом приложении выбери микрофон **`vois.rs`**

### Планы / идеи

- Виртуальный микрофон на Windows/macOS (VB-CABLE / BlackHole) как устройство вывода
- Замер задержки и режимы FFT с меньшей латентностью
- Автокоррекция высоты / сглаживание в стиле караоке
- Больше пресетов и пользовательские цепочки эффектов
- Web-UI / трей-иконка
- Звуковая плата (soundboard) через виртуальный микрофон

---

### Links

- [Releases](../../releases)
- [Issues](../../issues)
- [License](LICENSE)

### License

GNU General Public License v3.0 (or later)
