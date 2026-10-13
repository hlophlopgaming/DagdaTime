<div align="center">

# DagdaTime

### Alternativa de código abierto y multiplataforma a ManicTime

**Seguimiento automático del tiempo que se queda en tu ordenador.**<br>
Mira en una línea de tiempo a dónde se fue tu día, asigna tiempo a proyectos y obtén informes honestos. Gratis y privado.

[![CI](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DagdaTime?style=social)](https://github.com/hlophlopgaming/DagdaTime/stargazers)

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · **Español** · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

---

> [!IMPORTANT]
> **DagdaTime está en una fase temprana de desarrollo.** El seguimiento de la ventana activa y del tiempo de inactividad ya funciona en Windows, macOS y Linux (X11). La línea de tiempo, las etiquetas y los informes son los siguientes pasos de la [hoja de ruta](#hoja-de-ruta).
> ⭐ Dale una **estrella** o activa 👀 **Watch** para seguir el progreso y enterarte del primer lanzamiento.

## ¿Por qué DagdaTime?

Abres el portátil a las 9 y de repente son las 18:00. ¿A dónde se fue el día?

**ManicTime** responde bien a esa pregunta, pero es de código cerrado y sus funciones más útiles requieren una licencia de pago. **DagdaTime** quiere ser el mismo tipo de herramienta, desarrollada en abierto:

- 🕒 **Seguimiento automático.** Registra la aplicación activa, el título de la ventana y si estabas frente al ordenador. Sin cronómetros que iniciar u olvidar.
- 📊 **Línea de tiempo.** Todo tu día en pistas paralelas: uso del ordenador, aplicaciones, documentos y sitios web, etiquetas.
- 🏷️ **Etiqueta el tiempo, no las tareas.** Selecciona un tramo en la línea de tiempo y asígnalo a un proyecto o cliente. Las reglas pueden hacerlo automáticamente.
- 🔒 **Privado por defecto.** Todo vive en un archivo SQLite local. Sin cuenta, sin nube, sin telemetría, sin conexiones de red.
- 💻 **Multiplataforma de verdad.** Windows, macOS y Linux desde una sola base de código en Rust, con soporte nativo para **Wayland** (GNOME, KDE, Sway/Hyprland), no solo XWayland.
- 🆓 **Gratis para siempre.** GPL-3.0. Sin versión «Pro».

## DagdaTime, ManicTime y ActivityWatch

|  | **DagdaTime** | ManicTime | ActivityWatch |
|---|---|---|---|
| Código abierto | ✅ GPL-3.0 | ❌ Propietario | ✅ MPL-2.0 |
| Precio | ✅ Gratis | Edición gratuita, Pro de pago | ✅ Gratis |
| Los datos se quedan en tu equipo | ✅ | ✅ | ✅ |
| Línea de tiempo con etiquetado manual de tramos | 🚧 Previsto (M5) | ✅ | Categorías por reglas |
| Reglas de etiquetado automático (app / título / URL) | 🚧 Previsto (M6) | ✅ | ✅ |
| Backends nativos para Wayland | 🚧 Previsto (M3) | — | Limitado |
| Importación desde ActivityWatch | 🚧 Previsto (M7) | ❌ | — |

El formato de eventos de DagdaTime es compatible con ActivityWatch, así que podrás traer tu historial.

## Hoja de ruta

- [x] **M0** Workspace de Rust, aplicación de escritorio con Tauri 2, CI en Windows / macOS / Linux
- [x] **M1** Almacenamiento SQLite con migraciones y fusión de eventos heartbeat
- [x] **M2** Watchers nativos: ventana activa y tiempo de inactividad en Windows, macOS, Linux X11
- [ ] **M3** Servicio en segundo plano, API local, detección de ausencia (AFK), Wayland (GNOME, KDE, wlroots)
- [ ] **M3.5** Estructura de la aplicación: ventana principal, navegación, estado del seguimiento, icono en la bandeja con pausa/reanudar
- [ ] **M4** Vista de día: línea de tiempo, selección de tramos, panel de resumen (como en ManicTime)
- [ ] **M5** Etiquetas: seleccionar un tramo en la línea de tiempo → asignar un proyecto
- [ ] **M6** Reglas de etiquetado automático
- [ ] **M7** Informes, exportación CSV/JSON, importación desde ActivityWatch
- [ ] **M9** Extensión de navegador para URL (Firefox, Chromium)
- [ ] **M10** Icono en la bandeja, inicio automático, ajustes de privacidad, instaladores (MSI, DMG, AppImage/Flatpak)
- [ ] **Más adelante** Capturas de pantalla, cifrado de la base de datos, sincronización entre dispositivos

Detalles: [PLAN.md](PLAN.md) · Decisiones de arquitectura: [docs/decisions.md](docs/decisions.md)

## Pruébalo (desarrolladores)

Todavía no hay instaladores. Puedes compilar desde el código fuente y ejecutar la herramienta de depuración, que muestra cada segundo la ventana activa y el tiempo de inactividad:

```sh
git clone https://github.com/hlophlopgaming/DagdaTime.git
cd DagdaTime
cargo run --package dagdatime-daemon --bin tracker -- debug
```

Requisitos de compilación y notas por plataforma: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (en inglés).

## Privacidad

Un rastreador de tiempo ve todo lo que haces, así que aquí no hay concesiones:

- Todos los datos se guardan localmente en un único archivo SQLite que te pertenece.
- DagdaTime **no hace conexiones de red externas** y **no tiene telemetría**.
- Previsto: excluir aplicaciones, ocultar títulos de ventanas, cifrado opcional de la base de datos.

## Preguntas frecuentes

<details>
<summary><b>¿Existe una alternativa gratuita y de código abierto a ManicTime?</b></summary>

Eso es exactamente lo que DagdaTime quiere ser: seguimiento automático, línea de tiempo y etiquetado del tiempo, gratis y con licencia GPL, en Windows, macOS y Linux.
</details>

<details>
<summary><b>¿Funciona en Linux y Wayland?</b></summary>

Sí, Linux es una plataforma de primera clase. X11 ya funciona. Los backends nativos de Wayland para GNOME, KDE y compositores wlroots (Sway, Hyprland) están previstos para M3.
</details>

<details>
<summary><b>¿También es una alternativa a RescueTime, Timing o WakaTime?</b></summary>

En parte. Como RescueTime y Timing, registra automáticamente el uso del ordenador, pero todo se queda en local y sin suscripción. A diferencia de WakaTime, no se limita a los editores de código.
</details>

<details>
<summary><b>¿Puedo importar mis datos de ManicTime o ActivityWatch?</b></summary>

La importación desde ActivityWatch está prevista para M7. La importación desde ManicTime está en estudio: abre un issue si la necesitas.
</details>

<details>
<summary><b>¿Envía mis datos a algún sitio?</b></summary>

No. No hay servidor, cuenta ni analítica. La sincronización entre dispositivos puede llegar más adelante, y será opcional.
</details>

## Contribuir

Las contribuciones son bienvenidas, sobre todo de personas que conozcan:

- compositores **Wayland** (extensiones de GNOME Shell, scripts de KWin, IPC de wlroots);
- las API de accesibilidad de **macOS**;
- diseño de **interfaces de línea de tiempo** y visualización de datos.

Abre un issue para hablar de una idea o elige un hito de [PLAN.md](PLAN.md). El proyecto se desarrolla con agentes de programación con IA; sus reglas de trabajo están en [AGENTS.md](AGENTS.md).

Si DagdaTime te parece útil, **una estrella ayuda a que otras personas lo encuentren.** ⭐

## Licencia

[GPL-3.0](LICENSE)

<sub>ManicTime, RescueTime, Timing y WakaTime son marcas de sus respectivos propietarios. DagdaTime es un proyecto independiente y no está afiliado a ellos.</sub>
