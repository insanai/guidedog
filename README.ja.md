# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**一度書く。明快に伝える。Web と紙に届ける。**

Guidedog は reStructuredText と Markdown から HTML 文書サイトと PDF の本を作ります。Rust で実装され、変換エンジンは **Guidedoc** と呼ばれます。PDF はプログラムに組み込まれた Typst で組版します。Python 環境や LaTeX は不要です。

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

API ドキュメントはソースを実行せずに読み取ります。対象は Python（`autodoc`）、Odin（`odindoc`）、そして 0.7 で新たに加わった Rust クレート（`rust` ドメインと `rust_autoapi_*` 設定）です。

## インストール

各[リリース](https://github.com/insanai/guidedog/releases)には Linux（x86-64 と ARM64）と macOS（Apple シリコンと Intel）向けのアーカイブがあります。`guidedog` アーカイブには `guidedog`、`dsc`、`guidedog-pages`、`graphdog` が、`dscc` アーカイブには `dscc` だけが入っています。Typst コンパイラーは組み込まれ、tree-sitter もプログラムにコンパイルされています。Graphviz はシステムのものをリンクするので、先にそのライブラリーをインストールしてください。Debian と Ubuntu では `sudo apt install libgvc6`（または `graphviz` パッケージ）、Fedora では `sudo dnf install graphviz`、macOS では `brew install graphviz` です。Linux には glibc 2.35 以降、macOS にはバージョン 12 以降が必要です。対象のシステム向けのアーカイブと `SHA256SUMS` をダウンロードし、確認してからインストールします。

```sh
sha256sum -c --ignore-missing SHA256SUMS      # macOS では: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog guidedog-linux-amd64/dsc \
    guidedog-linux-amd64/guidedog-pages guidedog-linux-amd64/graphdog /usr/local/bin/
```

macOS では、公式の [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog) からインストールします。Homebrew が Apple シリコンまたは Intel 用のアーカイブを選び、Graphviz も一緒にインストールします。この tap は Linux にも対応しています。

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

プログラムにはコード署名がありません。macOS でブラウザーからダウンロードした場合は、`xattr -d com.apple.quarantine guidedog` で隔離属性を外してください。各プラットフォームの制限はリリースノートに書いてあります。

## ソースからビルドする

Rust 1.90 以降（[rustup](https://rustup.rs) で入手）、C コンパイラー（tree-sitter の文法は C で書かれています）、Graphviz の開発ファイルを用意します。Debian と Ubuntu では `libgraphviz-dev`、Fedora では `graphviz-devel`、macOS では Homebrew の `graphviz` です。そのうえでワークスペースをビルドします。

```sh
cargo build --release -p guidedog-cmd -p dsc-cmd -p dscc-cmd -p guidedog-pages-cmd -p graphdog-cmd
target/release/guidedog --version
```

プログラムは `target/release` にできます。Homebrew を使う macOS では、先にリンカーに Graphviz の場所を教えてください: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`。Typst コンパイラーは既定で組み込まれます（`gd-typst` の `embedded` フィーチャー）。これを外すと、Guidedog は PDF の生成にインストール済みの `typst` 0.15.1 を使います。Linux では、組み込みコンパイラーのパッケージダウンローダーが同梱の OpenSSL をリンクするため、そのビルドに `perl` と `make` が必要です。

変更を送る前に、CI と同じものを実行してください。

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # ソースの制限: 関数とファイルの長さ、行の幅
```

リリースの手順は [RELEASING.md](RELEASING.md) にあります。

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

PO ファイルの `msgstr` を翻訳し、内容を確認します。Guidedog は POT 抽出、カタログのマージ、進捗表示、各版のビルドを行いますが、翻訳文を自動で作りません。コマンド、パス、コード、API 名、参照先は変えません。翻訳するのはマニュアルとリポジトリ概要のみで、API と GDS は英語のままです。`cargo xtask build-site` は、すべての版とその PDF の本、そして議論の記録をウェブサイトにまとめます。

GitHub では上のリンクから README の言語を選びます。言語の自動選択は生成した文書サイトで行います。

## GitHub Pages に公開する

`guidedog-pages` は、手元のマシンからサイトを公開します。インストール済みの `guidedog` でプロジェクトをビルドし、サイト全体を 1 回の Git プッシュで専用のブランチに送ります。GitHub がこのプッシュを反映するのは、読み取ったときのコミットをブランチがまだ指している場合だけです。そのあと、GitHub Pages がそのコミットをデプロイするまで待ちます。トークンはシステムの認証情報ストアに保管され、プロファイルがどこに公開できるかは、マシン上の紐付け（bind）で決まります。

```sh
guidedog-pages init --repository=octo/manual --project=docs
guidedog-pages auth import          # fine-grained トークン。入力は表示されない
guidedog-pages bind                 # 公開先を確認し、このマシンに許可する
guidedog-pages publish
guidedog-pages setup                # ブランチから GitHub Pages をデプロイする
guidedog-pages watch                # ソースが変わるたびに公開し直す
```

もう一つの方法は GitHub Actions で、プッシュのたびに公開できます。`guidedog publish --publisher=pages` は Guidedog 自身のエンジンでビルドし、同じパブリッシャーを実行します。[GitHub Pages に公開する](docs/manual/howto/github-pages.rst)と[プラグインを書く](docs/manual/plugins.rst)を参照してください。

ビルドプラグインは、型付きでバージョン管理された契約を通じて、ソース、HTML ページ全体、静的アセットを変換します。プラグインは普通の Rust クレートです。`gd_cli::run_with` で独自の `guidedog` にリンクすることも、`guidedog plugin register` が SHA-256 で固定する別プログラムとして実行することもできます。`guidedog publish --timeout=SECONDS` はパブリッシャーとのやり取り全体の期限です。既定は 300 秒で、`--wait` より長くします。結果が不明なら再公開の前に復旧してください。

## 議論を管理する

`dsc` は提案と決定を reStructuredText の記録として 1 件 1 ファイルで保管し、Guidedog でウェブサイトと PDF の本にビルドします。カタログにはそれぞれプレフィックスがあります。Guidedog の設計記録は `gds` を使い、別のプロジェクトなら `eds` などを使います。

```sh
dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
dsc new "Cache policy" --labels=architecture
dsc promote cache-policy
dsc state 1 --to=accepted --reason="Review converged"
dsc build
```

カタログの `discussion.toml` は、ライフサイクルの状態、記録の種類、番号付けを設定し、`dsc config` はその内容を表示します。`dsc` は Guidedog のアーカイブに含まれ、インストール済みの `guidedog` でビルドします。`dscc` は Guidedog を組み込んだ `dsc` で、ほかに何もインストールされていないマシン向けです。各リリースには全プラットフォームの `dscc` アーカイブがあります。`guidedog gds` は Guidedog 自身の `docs/gds` に対して同じコマンドを実行します。[議論カタログの管理](docs/manual/howto/discussions.rst)を参照してください。

`graphdog` は、Guidedog の図と同じライブラリーで Graphviz DOT を SVG、PNG、またはレイアウト済みの DOT に描画します。`dot` コマンドのサブセットとして動きます: `echo 'digraph { a -> b }' | graphdog -Tsvg -o graph.svg`。

## 読む・組み込む

- [マニュアル](docs/manual/index.rst): チュートリアル、解説、作業手順、リファレンス、内部構造。
- [API リファレンス](docs/api/index.rst): Guidedog 自身が Rust ソースから生成し、マニュアル PDF の付録にも含めます。`cargo doc --workspace --no-deps` で rustdoc のページもビルドできます。
- [エンジンガイド](crates/guidedoc/README.md): ワークスペース、リクエスト、アダプター、パス。[例](examples/README.md)は `cargo run` で実行できる小さなプログラムです。
- [テンプレート](docs/manual/templates.rst): Jinja で HTML、Typst で本を調整します。
- [GDS](docs/gds/README.md): 番号付きの設計記録と実装上の判断。
- [議論ライブラリ](crates/gd-discussion/README.md): `dsc`、`dscc`、`guidedog gds` が管理するカタログ。
- [プラグインライブラリ](crates/gd-plugins/README.md): 型付きのビルドサービスと、`guidedog publish` と `guidedog-pages` が使うパブリッシャーインターフェース。
- [再利用できるクレート](docs/manual/internals/crates.rst): 他の Rust プロジェクトが crates.io から使えるエンジン、ライブラリ、API ジェネレーター、プラグイン SDK と、それらの依存関係。

`crates/` の各クレートには、API と移植時の設計判断をまとめた短い README があります。Guidedoc は文書のテーブルを自分で所有し、変換の間で再利用するので、定常状態の変換はメモリを確保しません。また、ヒント付きの構造化された診断を返します。ホストがファイル、予算、キャッシュ、公開、組み込みコンパイラーを管理します。`--untrusted` は機能を制限しますが、プロセスのサンドボックスでも、すべてのネイティブ資源の上限でもありません。ベータ版の対象は macOS と Linux の信頼できる文書プロジェクトです。

Guidedog 0.6 以前は Odin で書かれていました。バージョン 0.7 は Rust への移植で、コマンド、オプション、`conf.toml` のキー、ディレクティブ、ロール、出力はそのままです。変更点は[変更履歴](CHANGELOG.md)にあります。

## 謝辞とライセンス

**Vikrant Rathore** が制作し、**Ronak Rathore** と **Kanak Rathore** が開発を支援しました。Issue や Pull Request による貢献を歓迎します。文書の著者表記には名前やハンドルを使い、個人のメールアドレスは掲載しません。Git の識別情報は別に管理します。

Sphinx と Georg Brandl、Docutils と David Goodger、Typst、Tree-sitter と Max Brunsfeld、Graphviz、Rust プロジェクトとそのコミュニティ、そして Guidedog の最初の実装を支えた言語の作者 Ginger Bill と Odin コミュニティに感謝します。

Guidedog は [GNU Affero General Public License v3.0 以降](LICENSE)（AGPL-3.0-or-later）で公開しています。別のライセンス条件をご希望の場合は、作者にご連絡ください。連絡方法と、Rust クレートや同梱フォントなど独自のライセンスが適用される第三者コンポーネントの一覧は [NOTICE](NOTICE) にあります。追加の許可により、Guidedog を Graphviz とリンクして配布できます。詳しくは [NOTICE](NOTICE) をご覧ください。
