<div align="center">

# DagdaTime

### Alternative open source et multiplateforme à ManicTime

**Un suivi du temps automatique qui reste sur votre ordinateur.**<br>
Voyez où est passée votre journée sur une timeline, associez votre temps à des projets et obtenez des rapports fiables. Gratuit et privé.

[![CI](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DagdaTime?style=social)](https://github.com/hlophlopgaming/DagdaTime/stargazers)

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · **Français** · [Español](README.es.md) · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

---

> [!IMPORTANT]
> **DagdaTime est en début de développement.** Le suivi de la fenêtre active et du temps d'inactivité fonctionne déjà sous Windows, macOS et Linux (X11). La timeline, les étiquettes et les rapports sont les prochaines étapes de la [feuille de route](#feuille-de-route).
> ⭐ Ajoutez une **étoile** ou activez 👀 **Watch** pour suivre l'avancement et être averti de la première version.

## Pourquoi DagdaTime ?

Vous ouvrez votre ordinateur à 9 h, et soudain il est 18 h. Où est passée la journée ?

**ManicTime** répond bien à cette question, mais son code est fermé et ses fonctions les plus utiles exigent une licence payante. **DagdaTime** veut être le même type d'outil, développé en open source :

- 🕒 **Suivi automatique.** Enregistre l'application active, le titre de la fenêtre et votre présence devant l'ordinateur. Aucun chronomètre à lancer ou à oublier.
- 📊 **Timeline.** Toute votre journée sur des pistes parallèles : utilisation de l'ordinateur, applications, documents et sites web, étiquettes.
- 🏷️ **Étiquetez le temps, pas les tâches.** Sélectionnez une plage sur la timeline et attribuez-la à un projet ou un client. Des règles peuvent le faire automatiquement.
- 🔒 **Privé par défaut.** Tout est stocké dans un fichier SQLite local. Pas de compte, pas de cloud, pas de télémétrie, aucun appel réseau.
- 💻 **Vraiment multiplateforme.** Windows, macOS et Linux à partir d'une seule base de code Rust, avec une prise en charge native de **Wayland** (GNOME, KDE, Sway/Hyprland), pas seulement XWayland.
- 🆓 **Gratuit pour toujours.** GPL-3.0. Pas de version « Pro ».

## DagdaTime, ManicTime et ActivityWatch

|  | **DagdaTime** | ManicTime | ActivityWatch |
|---|---|---|---|
| Open source | ✅ GPL-3.0 | ❌ Propriétaire | ✅ MPL-2.0 |
| Prix | ✅ Gratuit | Édition gratuite, Pro payante | ✅ Gratuit |
| Les données restent sur votre machine | ✅ | ✅ | ✅ |
| Timeline avec étiquetage manuel de plages horaires | 🚧 Prévu (M5) | ✅ | Catégories par règles |
| Règles d'étiquetage automatique (app / titre / URL) | 🚧 Prévu (M6) | ✅ | ✅ |
| Backends Wayland natifs | ✅ GNOME/KDE/wlroots | — | Limité |
| Import depuis ActivityWatch | 🚧 Prévu (M7) | ❌ | — |

Le format d'événements de DagdaTime est compatible avec ActivityWatch : vous pourrez emporter votre historique.

## Feuille de route

- [x] **M0** Workspace Rust, interface de bureau Tauri 2, CI sous Windows / macOS / Linux
- [x] **M1** Stockage SQLite avec migrations et fusion des événements heartbeat
- [x] **M2** Watchers natifs : fenêtre active et temps d'inactivité sous Windows, macOS, Linux X11
- [x] **M3** Service en arrière-plan, API locale, détection d'absence (AFK), Wayland (GNOME, KDE, wlroots)
- [x] **M3.5** Squelette de l'application : fenêtre principale, navigation, état du suivi, icône dans la barre système avec pause/reprise
- [x] **M4** Vue Jour : timeline, sélection de plages, panneau récapitulatif (comme dans ManicTime)
- [ ] **M5** Étiquettes : sélectionner une plage sur la timeline → attribuer un projet
- [ ] **M6** Règles d'étiquetage automatique
- [ ] **M7** Rapports, export CSV/JSON, import depuis ActivityWatch
- [ ] **M9** Extension de navigateur pour les URL (Firefox, Chromium)
- [ ] **M10** Icône dans la barre système, démarrage automatique, réglages de confidentialité, installateurs (MSI, DMG, AppImage/Flatpak)
- [ ] **Plus tard** Captures d'écran, chiffrement de la base, synchronisation entre appareils

Détails : [PLAN.md](PLAN.md) · Décisions d'architecture : [docs/decisions.md](docs/decisions.md)

## Essayer (développeurs)

Il n'y a pas encore d'installateur. Vous pouvez compiler depuis les sources et lancer l'outil de débogage, qui affiche chaque seconde la fenêtre active et le temps d'inactivité :

```sh
git clone https://github.com/hlophlopgaming/DagdaTime.git
cd DagdaTime
npm ci --prefix app/frontend
npm --prefix app/frontend run build
cargo run --package dagdatime-app
```

Prérequis de compilation et notes par plateforme : [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (en anglais).

## Confidentialité

Un outil de suivi du temps voit tout ce que vous faites, donc aucun compromis ici :

- Toutes les données sont stockées localement dans un seul fichier SQLite qui vous appartient.
- DagdaTime n'effectue **aucun appel réseau externe** et ne collecte **aucune télémétrie**.
- Prévu : exclusion d'applications, masquage des titres de fenêtres, chiffrement optionnel de la base.

## FAQ

<details>
<summary><b>Existe-t-il une alternative gratuite et open source à ManicTime ?</b></summary>

C'est exactement l'objectif de DagdaTime : suivi automatique, timeline et étiquetage du temps, gratuit et sous licence GPL, sous Windows, macOS et Linux.
</details>

<details>
<summary><b>Fonctionne-t-il sous Linux et Wayland ?</b></summary>

Oui, Linux est une plateforme à part entière. X11 fonctionne déjà. Les backends Wayland natifs pour GNOME, KDE et les compositeurs wlroots (Sway, Hyprland) sont prévus pour M3.
</details>

<details>
<summary><b>Est-ce aussi une alternative à RescueTime, Timing ou WakaTime ?</b></summary>

En partie. Comme RescueTime et Timing, il suit automatiquement votre utilisation de l'ordinateur, mais tout reste en local et sans abonnement. Contrairement à WakaTime, il ne se limite pas aux éditeurs de code.
</details>

<details>
<summary><b>Puis-je importer mes données ManicTime ou ActivityWatch ?</b></summary>

L'import depuis ActivityWatch est prévu pour M7. L'import depuis ManicTime est à l'étude : ouvrez une issue si vous en avez besoin.
</details>

<details>
<summary><b>Mes données sont-elles envoyées quelque part ?</b></summary>

Non. Pas de serveur, pas de compte, pas d'analytics. Une synchronisation entre appareils pourra arriver plus tard, et elle sera optionnelle.
</details>

## Contribuer

Les contributions sont les bienvenues, en particulier de la part de personnes qui connaissent :

- les compositeurs **Wayland** (extensions GNOME Shell, scripts KWin, IPC wlroots) ;
- les API d'accessibilité de **macOS** ;
- le design d'**interfaces de timeline** et la visualisation de données.

Ouvrez une issue pour discuter d'une idée, ou choisissez une étape dans [PLAN.md](PLAN.md). Le projet est développé avec des agents de programmation IA ; leurs règles de travail sont dans [AGENTS.md](AGENTS.md).

Si DagdaTime vous semble utile, **une étoile aide d'autres personnes à le trouver.** ⭐

## Licence

[GPL-3.0](LICENSE)

<sub>ManicTime, RescueTime, Timing et WakaTime sont des marques de leurs propriétaires respectifs. DagdaTime est un projet indépendant, sans lien avec eux.</sub>
