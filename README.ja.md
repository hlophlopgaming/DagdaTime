<div align="center">

# DagdaTime

### オープンソースでクロスプラットフォームな ManicTime の代替

**データがあなたのパソコンから出ない、自動タイムトラッキング。**<br>
1日の時間がどこに消えたのかをタイムラインで確認し、時間をプロジェクトに割り当て、正直なレポートを得られます。無料でプライベート。

[![CI](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DagdaTime?style=social)](https://github.com/hlophlopgaming/DagdaTime/stargazers)

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · **日本語**

</div>

---

> [!IMPORTANT]
> **DagdaTime は開発の初期段階です。** アクティブウィンドウとアイドル時間の記録は、Windows・macOS・Linux (X11) ですでに動作しています。タイムライン、タグ、レポートは[ロードマップ](#ロードマップ)の次のステップです。
> ⭐ **Star** を付けるか 👀 **Watch** を有効にして、進捗と最初のリリースをチェックしてください。

## なぜ DagdaTime？

朝9時にパソコンを開いたら、気づけばもう18時。今日の時間はどこへ行ったのでしょう？

**ManicTime** はこの疑問にうまく答えてくれますが、クローズドソースで、便利な機能の多くは有料ライセンスが必要です。**DagdaTime** は同じタイプのツールを、オープンに開発することを目指しています：

- 🕒 **自動トラッキング。** アクティブなアプリ、ウィンドウタイトル、そしてパソコンの前にいたかどうかを記録します。タイマーを開始したり、止め忘れたりする必要はありません。
- 📊 **タイムライン表示。** 1日全体を並列のトラックで表示：パソコンの使用状況、アプリ、ドキュメントとWebサイト、タグ。
- 🏷️ **タスクではなく時間にタグを。** タイムライン上で範囲を選択し、プロジェクトやクライアントに割り当てます。ルールで自動化することもできます。
- 🔒 **デフォルトでプライベート。** すべてのデータはローカルの SQLite ファイルに保存されます。アカウント不要、クラウドなし、テレメトリなし、ネットワーク通信なし。
- 💻 **本当のクロスプラットフォーム。** 1つの Rust コードベースで Windows・macOS・Linux に対応。XWayland だけでなく、ネイティブの **Wayland**（GNOME、KDE、Sway/Hyprland）もサポートします。
- 🆓 **ずっと無料。** GPL-3.0。「Pro」版はありません。

## DagdaTime・ManicTime・ActivityWatch の比較

|  | **DagdaTime** | ManicTime | ActivityWatch |
|---|---|---|---|
| オープンソース | ✅ GPL-3.0 | ❌ プロプライエタリ | ✅ MPL-2.0 |
| 価格 | ✅ 無料 | 無料版 + 有料 Pro 版 | ✅ 無料 |
| データは自分のマシンに保存 | ✅ | ✅ | ✅ |
| タイムライン上での時間範囲の手動タグ付け | 🚧 予定 (M5) | ✅ | ルールベースのカテゴリ |
| 自動タグ付けルール（アプリ / タイトル / URL） | 🚧 予定 (M6) | ✅ | ✅ |
| ネイティブ Wayland バックエンド | ✅ GNOME/KDE/wlroots | — | 限定的 |
| ActivityWatch からのインポート | 🚧 予定 (M7) | ❌ | — |

DagdaTime のイベント形式は ActivityWatch と互換性があるため、これまでの履歴を引き継げます。

## ロードマップ

- [x] **M0** Rust ワークスペース、Tauri 2 デスクトップシェル、Windows / macOS / Linux での CI
- [x] **M1** マイグレーション対応の SQLite ストレージと、ハートビートイベントの統合
- [x] **M2** ネイティブウォッチャー：Windows・macOS・Linux X11 でのアクティブウィンドウとアイドル時間
- [x] **M3** バックグラウンドデーモン、ローカル API、離席検出 (AFK)、Wayland（GNOME、KDE、wlroots）
- [x] **M3.5** アプリの骨組み：メインウィンドウ、ナビゲーション、記録ステータス、一時停止/再開付きのトレイアイコン
- [x] **M4** 日表示：タイムライン、範囲選択、サマリーパネル（ManicTime と同様）
- [ ] **M5** タグ：タイムライン上で範囲を選択 → プロジェクトを割り当て
- [ ] **M6** 自動タグ付けルール
- [ ] **M7** レポート、CSV/JSON エクスポート、ActivityWatch からのインポート
- [ ] **M9** URL 記録用のブラウザ拡張機能（Firefox、Chromium）
- [ ] **M10** トレイアイコン、自動起動、プライバシー設定、インストーラー（MSI、DMG、AppImage/Flatpak）
- [ ] **その後** スクリーンショット、データベース暗号化、複数デバイス間の同期

詳細：[PLAN.md](PLAN.md) · アーキテクチャ上の決定：[docs/decisions.md](docs/decisions.md)

## 試してみる（開発者向け）

インストーラーはまだありません。ソースからビルドして、アクティブウィンドウとアイドル時間を1秒ごとに表示するデバッグツールを実行できます：

```sh
git clone https://github.com/hlophlopgaming/DagdaTime.git
cd DagdaTime
npm ci --prefix app/frontend
npm --prefix app/frontend run build
cargo run --package dagdatime-app
```

ビルド要件とプラットフォームごとの注意点：[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)（英語）。

## プライバシー

タイムトラッカーはあなたの行動をすべて見ています。だからこそ、ここは妥協しません：

- すべてのデータは、あなたが所有する1つの SQLite ファイルにローカル保存されます。
- DagdaTime は**外部へのネットワーク通信を一切行わず**、**テレメトリもありません**。
- 予定：特定アプリの除外、ウィンドウタイトルのマスク、オプションのデータベース暗号化。

## よくある質問

<details>
<summary><b>ManicTime の無料でオープンソースな代替はありますか？</b></summary>

まさにそれが DagdaTime の目標です。自動トラッキング、タイムライン、時間のタグ付けを、無料かつ GPL ライセンスで、Windows・macOS・Linux に提供します。
</details>

<details>
<summary><b>Linux や Wayland で動きますか？</b></summary>

はい、Linux は主要なプラットフォームです。X11 はすでに動作しています。GNOME、KDE、Sway、Hyprland 向けの Wayland バックエンドを実装済みです。GNOME/KDE はローカル統合が必要です。[Wayland](docs/wayland.md) を参照してください。実環境での手動確認が必要です。
</details>

<details>
<summary><b>RescueTime、Timing、WakaTime の代わりにもなりますか？</b></summary>

部分的には。RescueTime や Timing と同じようにパソコンの使用状況を自動で記録しますが、すべてローカルに保存され、サブスクリプションも不要です。WakaTime と違い、コードエディタに限定されません。
</details>

<details>
<summary><b>ManicTime や ActivityWatch のデータをインポートできますか？</b></summary>

ActivityWatch からのインポートは M7 で予定しています。ManicTime からのインポートは検討中です。必要な場合は issue を作成してください。
</details>

<details>
<summary><b>データがどこかに送信されることはありますか？</b></summary>

ありません。サーバーもアカウントもアナリティクスもありません。複数デバイス間の同期は将来追加されるかもしれませんが、その場合もオプトインになります。
</details>

## コントリビュート

コントリビューションを歓迎します。特に次の分野に詳しい方：

- **Wayland** コンポジター（GNOME Shell 拡張、KWin スクリプト、wlroots IPC）
- **macOS** のアクセシビリティ API
- **タイムライン UI** のデザインとデータ可視化

アイデアを話し合うには issue を作成するか、[PLAN.md](PLAN.md) からマイルストーンを選んでください。このプロジェクトは AI コーディングエージェントを使って開発されており、その作業ルールは [AGENTS.md](AGENTS.md) にあります。

DagdaTime が役に立ちそうだと思ったら、**Star が他の人に見つけてもらう助けになります。** ⭐

## ライセンス

[GPL-3.0](LICENSE)

<sub>ManicTime、RescueTime、Timing、WakaTime はそれぞれの所有者の商標です。DagdaTime は独立したプロジェクトであり、これらとは一切関係ありません。</sub>
