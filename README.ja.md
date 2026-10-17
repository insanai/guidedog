# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**一度書く。明快に伝える。Web と紙に届ける。**

Guidedog は、同じ reStructuredText または Markdown ファイルから文書サイトと PDF の本を作ります。ひとつの原稿を管理し、Web と紙の読者に届けます。配布版には PDF の組版と図の描画を組み込んだ、ひとつの実行ファイル `guidedog` が含まれます。

執筆の流れは Sphinx を参考にしています。ただし Guidedog は独立したシステムです。設定は宣言的な TOML、API 抽出は静的解析で行い、Python の設定や拡張を実行しません。Sphinx との完全互換は目標ではありません。

## プロジェクトを始める

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

プロジェクトには `conf.toml`、`index.rst`、`_templates/layout.html`、`_templates/book.typ`、`_static/` があります。テンプレートは、手元で編集できる普通のファイルです。HTML にはナビゲーション、検索、コードの強調、図、数式があり、PDF には章、相互参照、図、索引、言語に応じた組版があります。

既定は明るいテーマで、暗いテーマやシステム設定も選べます。マニュアルの入口とリポジトリ概要ではブラウザーの言語に合わせます。保存した選択が優先され、特定の版や章への直接リンクは尊重されます。

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API ドキュメントはソースコードを実行せずに読み取ります。対象は Python（`autodoc`）、Odin（`odindoc`）、Rust クレート（`rust` ドメインと `rust_autoapi_*` 設定）です。

## インストール

各[リリース](https://github.com/insanai/guidedog/releases)には、Linux（x86-64・ARM64）と macOS（Apple シリコン・Intel）向けのアーカイブ、および Windows x86-64 向けの ZIP が用意されています。各アーカイブには実行ファイル `guidedog` とライセンス表記が含まれます。GitHub Pages への公開、静的サイトの公開、議論の管理はリンク済みプラグインとして組み込まれており、`gd-graphdog` は Graphviz をライブラリとして内包しています。Typst コンパイラーの組み込み、tree-sitter のコンパイル時統合、Graphviz の静的リンクが済んでいるため、追加でインストールするものは何もありません。Windows 版 ZIP には Graphviz の DLL が同梱されているため、展開したフォルダー一式をそのまま同じ場所に置いて使います。Linux は glibc 2.35 以降、macOS はバージョン 12 以降が必要です。システムに合ったアーカイブと `SHA256SUMS` をダウンロードし、検証してインストールします。

```sh
sha256sum -c --ignore-missing SHA256SUMS      # macOS では: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog /usr/local/bin/
```

macOS では、公式の [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog) からインストールできます。Homebrew が Apple シリコン用または Intel 用のアーカイブを自動で選びます。この tap は Linux にも対応しています。

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

実行ファイルにはコード署名がありません。macOS でブラウザーからダウンロードした場合は、`xattr -d com.apple.quarantine guidedog` で隔離属性を外してください。各プラットフォームの制限事項はリリースノートに記載しています。

## ソースからビルドする

Rust 1.99 以降（[rustup](https://rustup.rs) で導入）、C コンパイラー（tree-sitter の文法が C で書かれているため）、および Graphviz の開発用ファイルを用意します。Debian と Ubuntu では `libgraphviz-dev`、Fedora では `graphviz-devel`、macOS では Homebrew の `graphviz` です（あるいはリリースビルドと同様に Graphviz を静的ビルドすることもできます。`cargo xtask graphviz` を実行してから `GUIDEDOG_GRAPHVIZ_STATIC=target/graphviz/16.1.0` を指定してビルドすれば、実行時に Graphviz は不要になります）。そのうえでワークスペースをビルドします。

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

実行ファイルは `target/release` に生成されます。Homebrew を使う macOS では、先にリンカーへ Graphviz の場所を指定してください: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`。Typst コンパイラーは既定で組み込まれます（`gd-typst` の `embedded` フィーチャー）。これを無効にした場合、Guidedog は PDF の生成にインストール済みの `typst` 0.15.1 を呼び出します。Linux では、組み込みコンパイラーのパッケージダウンローダーが同梱の OpenSSL をリンクするため、ビルドに `perl` と `make` が必要です。

変更を送る前に、CI と同じチェックを実行してください。

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # ソースの制限: 関数とファイルの長さ、行幅
```

リリースの手順は [RELEASING.md](RELEASING.md) に記載しています。

## Guidedog で翻訳する

```sh
mkdir -p docs/manual/_editions
guidedog build -b gettext docs/manual docs/manual/locales/templates -W
guidedog intl update -p docs/manual/locales/templates \
    -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog build -b html docs/manual docs/manual/_editions/de \
    -D language=de -D 'html_extra_path=[]' -W
```

PO ファイルの `msgstr` を翻訳し、内容を確認します。Guidedog は POT 抽出、カタログのマージ、進捗表示、各言語版のビルドを行いますが、翻訳文を自動生成することはありません。コマンド、パス、コード、API 名、参照ターゲットはそのまま維持します。翻訳対象はマニュアルとリポジトリ概要のみで、API と GDS は英語のままです。`cargo xtask build-site` は、すべての言語版とその PDF の本、および議論の記録をウェブサイトにまとめます。

GitHub 上では冒頭のリンクから README の言語を選べます。言語の自動選択は生成された文書サイト側で行われます。

## GitHub Pages に公開する

`guidedog publish pages` は組み込みの GitHub Pages プラグインを使ってサイトを公開します。Guidedog 自身のエンジンでビルドし、未完了の設定があれば案内したうえで、完成した一世代分のサイトを公開します。`guidedog pages` では設定、認証情報、復旧、ワークフロー生成を扱えます。別のパブリッシャー実行ファイルをインストールする必要はありません。

```sh
guidedog publish pages --repository=octo/manual --project=docs
guidedog pages status
guidedog pages workflow --write
```

スクリプトでセットアップする場合は `guidedog pages init`、`auth import`、`bind`、`setup` を使います。生成したワークフローを使えば、プッシュのたびに GitHub Actions でビルドとデプロイを行えます。詳しくは [GitHub Pages に公開する](docs/manual/howto/github-pages.rst)を参照してください。

ビルドプラグインは、型付きでバージョン管理された契約を通じて、ソーステキスト、完成した HTML ページ、静的アセットを変換します。プラグインは普通の Rust クレートです。`gd_cli::run_with` で独自の `guidedog` にリンクすることも、`guidedog plugin register` で SHA-256 を固定した別プログラムとして実行することもできます。プラグインから `guidedog` 自体にサブコマンドを追加することもでき、追加されたコマンドは `guidedog --help` に表示されます。`guidedog publish --timeout=SECONDS` はパブリッシャーとの一連のやり取り全体の上限時間を秒単位で指定します（既定は 300 秒）。`--wait` より長い値を指定してください。結果が不明な状態で終わった場合は、再公開の前に復旧を行ってください。

## 議論を管理する

`guidedog dsc` は提案と決定を reStructuredText の記録として 1 件 1 ファイルで管理し、ウェブサイトと PDF の本にビルドします。各カタログにはプレフィックスがあり、Guidedog の設計記録では `gds` を使い、別のプロジェクトなら `eds` などを使います。

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

カタログの `discussion.toml` でライフサイクルの状態、記録の種類、番号付けを設定し、`guidedog dsc config` でその内容を表示できます。これらのコマンドはすべての `guidedog` にリンクされている議論プラグインが提供するため、追加でインストールするものは何もありません。Guidedog 自身の `docs/gds` もそうしたカタログの一つで、そのフォルダーで `guidedog dsc check` を実行しています。[議論カタログの管理](docs/manual/howto/discussions.rst)を参照してください。

`gd-graphdog` は Graphviz を内包するライブラリです。Guidedog はこれを使ってプロセス内でドキュメントの図を描画しており、独立した CLI ではありません。

## 静的サイトを公開する

`guidedog ssg` は、同じ `conf.toml` とドキュメントツリーから、記事、研究ログ、コレクション、フィード、ピン留めされたテーマアーカイブ、およびマルチプロジェクトのウェブサイトを管理します。

```sh
guidedog ssg init site --preset=journal --title="Field Notes"
guidedog ssg new posts/2026-04-17-zero-copy.rst --title="Zero-Copy Readers"
guidedog ssg schedule posts/2026-04-17-zero-copy.rst --at=2026-05-01T09:00:00Z
guidedog ssg publish posts/2026-04-17-zero-copy.rst
guidedog ssg build site
```

`:pdf: article` を指定したページは、HTML ページと並んで印刷品質の単体 PDF も生成されます。マルチプロジェクトのウェブサイトでは、`[[site.mounts]]` を使ってサブプロジェクト（`docs/manual`、`docs/api`、`docs/gds` など）を宣言的にマウントできます。[静的サイトの公開](docs/manual/howto/static-publishing.rst)を参照してください。

## 読む・組み込む

- [マニュアル](docs/manual/index.rst): チュートリアル、トピック解説、実践ガイド、リファレンス、内部構造。
- [API リファレンス](docs/api/index.rst): Guidedog 自身が Rust ソースから生成し、マニュアル PDF の付録にも収録しています。`cargo doc --workspace --no-deps` で rustdoc のページもビルドできます。
- [エンジンガイド](crates/guidedoc/README.md): ワークスペース、リクエスト、アダプター、パスについて。[サンプル](examples/README.md)は `cargo run` で実行できる小さなプログラムです。
- [テンプレート](docs/manual/templates.rst): Jinja で HTML を、Typst で本をカスタマイズする方法。
- [GDS](docs/gds/README.md): 番号付きの設計記録と実装上の決定事項。
- [議論ライブラリ](crates/gd-discussion/README.md): `guidedog dsc` が管理するカタログ。
- [プラグインライブラリ](crates/gd-plugins/README.md): 型付きのビルドサービスと、`guidedog publish` およびリンク済み Pages プラグインが使うパブリッシャーインターフェース。
- [再利用可能なクレート](docs/manual/internals/crates.rst): 他の Rust プロジェクトが crates.io から利用できるエンジン、ライブラリ、API ジェネレーター、プラグイン SDK と、それらの依存関係。

`crates/` 配下の各クレートには、API と設計判断をまとめた短い README があります。Guidedoc は文書のテーブルを自ら所有して変換間で再利用するため、定常状態の変換ではメモリアロケーションが発生しません。また、ヒント付きの構造化された診断情報を返します。ホスト側がファイル、バジェット、キャッシュ、公開、組み込みコンパイラーを管理します。`--untrusted` は機能を制限しますが、プロセスのサンドボックスではなく、あらゆるネイティブ資源の使用量に上限を設けるものでもありません。ベータ版の対象範囲は、Linux、macOS、Windows 上の信頼できるドキュメントプロジェクトです。

## 謝辞とライセンス

**Vikrant Rathore** が制作し、**Ronak Rathore** と **Kanak Rathore** が開発を支援しました。Issue や Pull Request による貢献を歓迎します。ドキュメントのクレジット表記には名前またはハンドルネームを使用し、個人のメールアドレスは記載しません。Git の識別情報は別途管理されます。

Sphinx と Georg Brandl、Docutils と David Goodger、Typst、Tree-sitter と Max Brunsfeld、Graphviz、そして Rust プロジェクトとそのコミュニティに感謝します。

Guidedog は [GNU Lesser General Public License v3.0 以降](LICENSE)（LGPL-3.0-or-later）のもとで公開されています。LGPL は [GNU General Public License v3.0](COPYING) に追加の許可を与えたものです。その他のライセンス条件については作者にお問い合わせください。連絡方法と、Rust クレートや同梱フォントなど独自のライセンスが維持されるサードパーティ製コンポーネントの一覧は [NOTICE](NOTICE) に記載されています。また、追加の許可により、Guidedog を Graphviz とリンクして配布することが認められています。詳しくは [NOTICE](NOTICE) をご覧ください。
