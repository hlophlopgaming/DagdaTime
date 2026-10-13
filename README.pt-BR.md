<div align="center">

# DagdaTime

### Alternativa de código aberto e multiplataforma ao ManicTime

**Controle de tempo automático que fica no seu computador.**<br>
Veja em uma linha do tempo para onde foi o seu dia, atribua tempo a projetos e tenha relatórios honestos. Gratuito e privado.

[![CI](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DagdaTime?style=social)](https://github.com/hlophlopgaming/DagdaTime/stargazers)

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · **Português** · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

---

> [!IMPORTANT]
> **O DagdaTime está no início do desenvolvimento.** O rastreamento da janela ativa e do tempo ocioso já funciona no Windows, macOS e Linux (X11). A linha do tempo, as tags e os relatórios são os próximos passos do [roadmap](#roadmap).
> ⭐ Dê uma **estrela** ou ative o 👀 **Watch** para acompanhar o progresso e saber do primeiro lançamento.

## Por que o DagdaTime?

Você abre o notebook às 9h e, de repente, já são 18h. Para onde foi o dia?

O **ManicTime** responde bem a essa pergunta, mas tem código fechado e os recursos mais úteis exigem uma licença paga. O **DagdaTime** quer ser o mesmo tipo de ferramenta, desenvolvida de forma aberta:

- 🕒 **Rastreamento automático.** Registra o aplicativo ativo, o título da janela e se você estava no computador. Sem cronômetros para iniciar ou esquecer.
- 📊 **Linha do tempo.** O seu dia inteiro em trilhas paralelas: uso do computador, aplicativos, documentos e sites, tags.
- 🏷️ **Marque o tempo, não as tarefas.** Selecione um trecho na linha do tempo e atribua a um projeto ou cliente. Regras podem fazer isso automaticamente.
- 🔒 **Privado por padrão.** Tudo fica em um arquivo SQLite local. Sem conta, sem nuvem, sem telemetria, sem chamadas de rede.
- 💻 **Multiplataforma de verdade.** Windows, macOS e Linux a partir de uma única base de código em Rust, com suporte nativo a **Wayland** (GNOME, KDE, Sway/Hyprland), não apenas XWayland.
- 🆓 **Gratuito para sempre.** GPL-3.0. Sem versão "Pro".

## DagdaTime, ManicTime e ActivityWatch

|  | **DagdaTime** | ManicTime | ActivityWatch |
|---|---|---|---|
| Código aberto | ✅ GPL-3.0 | ❌ Proprietário | ✅ MPL-2.0 |
| Preço | ✅ Gratuito | Edição gratuita, Pro paga | ✅ Gratuito |
| Os dados ficam na sua máquina | ✅ | ✅ | ✅ |
| Linha do tempo com marcação manual de intervalos | 🚧 Planejado (M5) | ✅ | Categorias por regras |
| Regras de marcação automática (app / título / URL) | 🚧 Planejado (M6) | ✅ | ✅ |
| Backends nativos para Wayland | ✅ GNOME/KDE/wlroots | — | Limitado |
| Importação do ActivityWatch | 🚧 Planejado (M7) | ❌ | — |

O formato de eventos do DagdaTime é compatível com o ActivityWatch, então você poderá trazer o seu histórico.

## Roadmap

- [x] **M0** Workspace Rust, aplicativo desktop com Tauri 2, CI no Windows / macOS / Linux
- [x] **M1** Armazenamento SQLite com migrações e mesclagem de eventos heartbeat
- [x] **M2** Watchers nativos: janela ativa e tempo ocioso no Windows, macOS, Linux X11
- [x] **M3** Serviço em segundo plano, API local, detecção de ausência (AFK), Wayland (GNOME, KDE, wlroots)
- [x] **M3.5** Estrutura do aplicativo: janela principal, navegação, status do rastreamento, ícone na bandeja com pausar/retomar
- [x] **M4** Visão do dia: linha do tempo, seleção de intervalos, painel de resumo (como no ManicTime)
- [ ] **M5** Tags: selecionar um trecho na linha do tempo → atribuir um projeto
- [ ] **M6** Regras de marcação automática
- [ ] **M7** Relatórios, exportação CSV/JSON, importação do ActivityWatch
- [ ] **M9** Extensão de navegador para URLs (Firefox, Chromium)
- [ ] **M10** Ícone na bandeja, inicialização automática, configurações de privacidade, instaladores (MSI, DMG, AppImage/Flatpak)
- [ ] **Depois** Capturas de tela, criptografia do banco de dados, sincronização entre dispositivos

Detalhes: [PLAN.md](PLAN.md) · Decisões de arquitetura: [docs/decisions.md](docs/decisions.md)

## Experimente (desenvolvedores)

Ainda não há instaladores. Você pode compilar a partir do código-fonte e rodar a ferramenta de depuração, que mostra a cada segundo a janela ativa e o tempo ocioso:

```sh
git clone https://github.com/hlophlopgaming/DagdaTime.git
cd DagdaTime
npm ci --prefix app/frontend
npm --prefix app/frontend run build
cargo run --package dagdatime-app
```

Requisitos de compilação e notas por plataforma: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (em inglês).

## Privacidade

Um rastreador de tempo vê tudo o que você faz, então aqui não há concessões:

- Todos os dados são armazenados localmente em um único arquivo SQLite que pertence a você.
- O DagdaTime **não faz chamadas de rede externas** e **não tem telemetria**.
- Planejado: excluir aplicativos, ocultar títulos de janelas, criptografia opcional do banco de dados.

## Perguntas frequentes

<details>
<summary><b>Existe uma alternativa gratuita e de código aberto ao ManicTime?</b></summary>

É exatamente isso que o DagdaTime quer ser: rastreamento automático, linha do tempo e marcação de tempo, gratuito e sob licença GPL, no Windows, macOS e Linux.
</details>

<details>
<summary><b>Funciona no Linux e no Wayland?</b></summary>

Sim, o Linux é uma plataforma de primeira classe. O X11 já funciona. Backends nativos de Wayland para GNOME, KDE e compositores wlroots (Sway, Hyprland) estão planejados para o M3.
</details>

<details>
<summary><b>Também é uma alternativa ao RescueTime, Timing ou WakaTime?</b></summary>

Em parte. Como o RescueTime e o Timing, ele registra automaticamente o uso do computador, mas tudo fica local e sem assinatura. Diferente do WakaTime, não se limita a editores de código.
</details>

<details>
<summary><b>Posso importar meus dados do ManicTime ou do ActivityWatch?</b></summary>

A importação do ActivityWatch está planejada para o M7. A importação do ManicTime está em avaliação: abra uma issue se precisar dela.
</details>

<details>
<summary><b>Ele envia meus dados para algum lugar?</b></summary>

Não. Não há servidor, conta nem analytics. A sincronização entre dispositivos pode vir depois, e será opcional.
</details>

## Contribuindo

Contribuições são bem-vindas, especialmente de quem conhece:

- compositores **Wayland** (extensões do GNOME Shell, scripts do KWin, IPC do wlroots);
- as APIs de acessibilidade do **macOS**;
- design de **interfaces de linha do tempo** e visualização de dados.

Abra uma issue para discutir uma ideia ou escolha um marco do [PLAN.md](PLAN.md). O projeto é desenvolvido com agentes de programação com IA; as regras de trabalho deles estão em [AGENTS.md](AGENTS.md).

Se o DagdaTime parece útil para você, **uma estrela ajuda outras pessoas a encontrá-lo.** ⭐

## Licença

[GPL-3.0](LICENSE)

<sub>ManicTime, RescueTime, Timing e WakaTime são marcas de seus respectivos proprietários. O DagdaTime é um projeto independente e não tem vínculo com eles.</sub>
