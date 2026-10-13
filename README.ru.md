<div align="center">

# Draugr Time

**Локальный учёт активности с нативным приложением на Rust.**

[![CI](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust + Iced](https://img.shields.io/badge/built%20with-Rust%20%2B%20Iced-orange)

[English](README.md) · **Русский** · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

Draugr Time записывает активное приложение, заголовок окна и время активности
или отсутствия, затем показывает их на дневном таймлайне. Это бесплатный
проект под GPL-3.0, вдохновлённый рабочим процессом ManicTime. История остаётся
на вашем компьютере.

> **Ранняя стадия разработки:** трекинг и дневной таймлайн уже работают.
> Редактирование тегов, правила, отчёты, импорт/экспорт и запись URL браузера
> пока запланированы.

## Что работает

- Нативный интерфейс **Iced** со светлой, тёмной и системной темами.
- Отслеживание активного окна и простоя на Windows, macOS и Linux X11.
- Бэкенды Wayland для GNOME, KDE, Sway и Hyprland. Для GNOME/KDE нужен локальный
  мост композитора: [настройка Wayland](docs/wayland.md).
- Дневной таймлайн: состояние компьютера, приложения, документы и теги;
  масштабирование, прокрутка, выделение диапазона и сводка длительности.
  Редактирование тегов появится в M5.
- Фоновая запись, управление из трея, пауза и автозапуск по желанию.
- Локальная SQLite-база и выбор папки данных.

Один исполняемый файл содержит интерфейс, трекинг, хранилище и localhost API.
Веб-фронтенда нет, отдельный сервис запускать не нужно. В обычном запуске
демо-режим отсутствует.

## Скриншоты

На скриншотах показаны сгенерированные примеры, а не личная история активности.

![Draugr Time — светлая тема](docs/screenshots/draugr-time-light.png)

![Draugr Time — тёмная тема](docs/screenshots/draugr-time-dark.png)

## Сборка и запуск

Установите стабильный Rust. На Linux нужны пакеты разработки GTK 3, Ayatana
AppIndicator, librsvg и libxkbcommon-x11. На Windows нужен MSVC, на macOS —
Xcode command-line tools. Готовых установщиков пока нет.

```sh
git clone https://github.com/hlophlopgaming/DraugrTime.git
cd DraugrTime
cargo run --release --package draugr-time
```

Исполняемый файл: `target/release/draugr-time` (`.exe` на Windows).
Нажмите **Start recording**, чтобы начать запись. Закрытие окна оставляет
приложение и запись в трее; **Quit** завершает работу.

[Инструкции для разработки](docs/DEVELOPMENT.md) описывают проверки и диагностику,
а [документация приложения](docs/desktop.md) — настройки и особенности платформ.

## Данные и приватность

- История хранится локально в SQLite. Нет аккаунта, облачной отправки и телеметрии.
- Сетевые обращения приложения ограничены localhost; IPC композитора тоже локальный.
- Новые установки используют системную папку данных `DraugrTime`. Существующая
  история может продолжать храниться в прежней папке `DagdaTime`.
- Settings → Data folder выбирает папку для следующего запуска. Существующая
  история остаётся на прежнем месте.

## Дорожная карта

- [x] **M0–M3.5:** workspace, хранилище, нативные коллекторы, локальный API и оболочка.
- [x] **M4:** дневной таймлайн, выделение диапазона и сводка.
- [ ] **M5:** редактирование тегов, менеджер тегов и разметка времени отсутствия.
- [ ] **M6:** автоматические правила тегирования.
- [ ] **M7:** недельный вид, отчёты, экспорт CSV/JSON и импорт ActivityWatch.
- [ ] **M9:** расширение Firefox/Chromium для URL браузера.
- [ ] **M10:** первый запуск, настройки приватности, резервные копии и установщики.

Подробности: [PLAN.md](PLAN.md) · [Архитектурные решения](docs/decisions.md).

## Участие в разработке

Обсудите изменение в [issue](https://github.com/hlophlopgaming/DraugrTime/issues).
Особенно полезна помощь с разрешениями ОС, интеграциями Wayland и проверкой
на реальных рабочих столах. Инструкции для агентов: [AGENTS.md](AGENTS.md).

## Лицензия

[GNU GPLv3](LICENSE). Draugr Time — независимый проект, не связанный с ManicTime
или ActivityWatch.
