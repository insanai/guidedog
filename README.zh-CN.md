# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**一次执笔，清晰阐述，同步呈现于网页与纸面。**

Guidedog 从同一组 reStructuredText 或 Markdown 文件生成文档网站和 PDF 图书。只需维护一套源文件，就能同时面向网页与纸面读者发布。发行版仅包含一个 `guidedog` 可执行文件，内置 PDF 排版、图表渲染与乐谱排版功能。

写作流程借鉴自 Sphinx，但 Guidedog 是一套独立系统：采用声明式 TOML 配置，通过静态分析提取 API，绝不执行任何 Python 配置或扩展，也不以完全兼容 Sphinx 为目标。

## 创建项目

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

项目包含 `conf.toml`、`index.rst`、`_templates/`（`layout.html`、`book.typ`、`discussion.typ`、`article.typ` 和 `theme.typ`）以及 `_static/`（`guidedog.css`、`guidedog.js` 和 `typography.js`）。模板和静态资源都是普通文件，可在本地直接编辑。HTML 提供导航、搜索、代码高亮、图表、公式及 Knuth-Plass 段落排版；PDF 图书提供章节、交叉引用、插图、索引及按语种优化的排版。可以在 RST 和 MyST 中直接嵌入 Graphviz 或 Typst 绘图源码（包括 CeTZ 与 Fletcher），自动生成适用于 HTML 与 PDF 的图形资源；参见[图表章节](docs/manual/topics/diagrams.rst)。

借助原生 `lilypond` 指令，还可在 RST 和 MyST 中直接编写 LilyPond 乐谱，支持谱表、每行谱表最多四个同时发声的声部、歌词、结构化 `\markup` 及常用记号；参见[乐谱章节](docs/manual/topics/music.rst)。转曲后的 Bravura 字体 SVG 和 PNG 谱例可直接用于 HTML 与多行谱表跨页排版的 PDF 图书，无需另行安装 LilyPond 程序：

```rst
.. lilypond::
   :caption: 贝多芬《欢乐颂》主题，C 大调四小节。
   :alt: 四小节旋律，以 E、F、G 开头，以附点四分音符 E、八分音符 D 和二分音符 D 结尾。

   \fixed c' {
     \clef treble
     \key c \major
     \time 4/4
     e2 f4 g | g4 f e d | c2 d4 e | e4. d8 d2 |
   }
```

![由 Guidedog 排版的贝多芬《欢乐颂》主题（C 大调四小节）](.github/ode-to-joy.svg)

内置主题默认跟随系统外观，并提供浅色与深色切换；Guidedog 自身的文档网站默认采用浅色阅读版。网站首页与仓库概览会自动检测浏览器语言，用户保存的语言偏好优先；直接指向特定语言版本或具体章节的链接，则按所指语言打开。

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API 文档直接从源码静态提取，无需运行代码：支持 Python（`autodoc`）、Odin（`odindoc`）以及 Rust crate（`rust` 域和 `rust_autoapi_*` 配置项）。

## 安装

每个[发行版](https://github.com/insanai/guidedog/releases)都提供 Linux（x86-64 与 ARM64）和 macOS（Apple 芯片与 Intel）归档包，以及 Windows x86-64 ZIP 包。每个归档包均包含 `guidedog` 可执行文件及许可声明。GitHub Pages 发布、静态站点发布和讨论记录均为内联链接的插件；`gd-graphdog` 将 Graphviz 封装为库。Typst 编译器与 Bravura 1.482 乐谱字体已内嵌，tree-sitter 已编译进程序，Graphviz 采用静态链接，因此无需额外部署其他依赖。Windows ZIP 包附带了 Graphviz DLL，解压后请保持整个文件夹完整。Linux 需要 glibc 2.35 或更高版本；macOS 需要 12 或更高版本。下载对应系统的归档包和 `SHA256SUMS`，校验后即可安装：

```sh
sha256sum -c --ignore-missing SHA256SUMS      # macOS 上：shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog /usr/local/bin/
```

在 macOS 上，可通过官方 [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog) 安装。Homebrew 会自动选择 Apple 芯片或 Intel 架构的归档包。该 tap 也支持 Linux：

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

可执行文件没有代码签名：在 macOS 上，请用 `xattr -d com.apple.quarantine guidedog` 解除浏览器下载文件的隔离属性。各平台的限制详见发行说明。

## 从源码构建

安装 Rust 1.99 或更高版本（通过 [rustup](https://rustup.rs)）、C 编译器（tree-sitter 语法分析器由 C 编写）以及 Graphviz 开发文件：Debian 和 Ubuntu 上为 `libgraphviz-dev`，Fedora 上为 `graphviz-devel`，macOS 上为 Homebrew 的 `graphviz`。（也可以像发行版那样静态构建 Graphviz：运行 `cargo xtask graphviz`，然后带 `GUIDEDOG_GRAPHVIZ_STATIC=target/graphviz/16.1.0` 构建；这样生成的程序在运行时就无需安装 Graphviz。）随后构建工作区：

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

生成的可执行文件位于 `target/release`。在使用 Homebrew 的 macOS 上，先让链接器找到 Graphviz：`export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`。Typst 编译器默认内嵌（即 `gd-typst` 的 `embedded` 特性）；若不启用该特性，Guidedog 会调用系统中安装的 `typst` 0.15.1 生成 PDF 图书，而 `.. typst-diagram::` 以及使用 `:format: png` 的 `.. lilypond::` 则需要内嵌编译器。在 Linux 上，内嵌编译器的包下载器会链接随源码附带的 OpenSSL，构建它需要 `perl` 和 `make`。

提交修改前，请运行与 CI 相同的检查：

```sh
cargo fmt --all --check
cargo run -p check              # 源码限制：函数与文件长度、行宽
cargo xtask tables --check      # 生成的 Unicode、实体与 SMuFL 表
cargo xtask check-music-font    # 固定版本的 Bravura 1.482 字体与元数据
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask fetch-fonts --music && cargo run -p music-check
```

[RELEASING.md](RELEASING.md) 介绍了发布流程。

## 用 Guidedog 翻译

```sh
mkdir -p docs/manual/_editions
guidedog build -b gettext docs/manual docs/manual/locales/templates -W
guidedog intl update -p docs/manual/locales/templates \
    -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog build -b html docs/manual docs/manual/_editions/de \
    -D language=de -D 'html_extra_path=[]' -W
```

在 PO 文件中翻译并审阅 `msgstr` 的值。Guidedog 负责提取 POT 文件、合并消息目录、报告进度并构建各语言版本，不会自行编造译文。命令、路径、代码、API 名称和引用目标保持不变。网站首页、手册和仓库概览均提供翻译，API 参考与 GDS 保持英文。`cargo xtask build-site` 会将所有语言版本、对应的 PDF 图书和讨论记录汇总组装成网站。

在 GitHub 上阅读时，请通过顶部链接切换 README 语言；自动语言切换仅在生成的文档网站上生效。

## 发布到 GitHub Pages

`guidedog publish pages` 通过内置的 GitHub Pages 插件进行发布。它使用 Guidedog 自身的引擎构建文档，引导完成缺失的配置，并整批发布新生成的全站文件。`guidedog pages` 提供配置、凭据、恢复和工作流生成子命令。无需安装独立的发布程序。

```sh
guidedog publish pages --repository=octo/manual --project=docs
guidedog pages status
guidedog pages workflow --write
```

在脚本化配置中，可以使用 `guidedog pages init`、`auth import`、`bind` 和 `setup`。借助生成的工作流，GitHub Actions 可以在每次推送后自动构建并部署。参见[发布到 GitHub Pages](docs/manual/howto/github-pages.rst)。

构建插件通过带类型、带版本的契约转换源文本、完整 HTML 页面和静态资源。插件是普通的 Rust crate。可以用 `gd_cli::run_with` 将其链接进自己的 `guidedog`，也可以将其作为独立程序运行，并由 `guidedog plugin register` 按 SHA-256 固定。插件还可以向 `guidedog` 本身添加命令，并显示在 `guidedog --help` 中。`guidedog publish --timeout=SECONDS` 限制与发布器的每次完整交互时间（默认 300 秒）；请设置比 `--wait` 更长的值。当发布结果不明时，请先执行恢复，再重新发布。

## 维护讨论

`guidedog dsc` 将提案和决策保存为 reStructuredText 记录（每条记录一个文件），并构建成网站和 PDF 图书。每个讨论目录都有前缀：Guidedog 的设计记录使用 `gds`，其他项目可以使用 `eds`。

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

目录的 `discussion.toml` 定义其生命周期状态、记录类型和编号规则，`guidedog dsc config` 可将其打印出来。这些命令来自讨论插件，每个 `guidedog` 都已链接该插件，因此无需安装任何额外组件。Guidedog 自身的 `docs/gds` 就是这样一个目录：可以在该目录下运行 `guidedog dsc check`。参见[维护讨论目录](docs/manual/howto/discussions.rst)。

`gd-graphdog` 是封装 Graphviz 的库。Guidedog 在进程内用它渲染文档图表；它不是独立的命令行程序。

## 发布静态站点

`guidedog ssg` 基于同一份 `conf.toml` 和文档树管理文章、研究日志、集合、订阅源、固定校验和的主题归档包以及多项目网站：

```sh
guidedog ssg init site --preset=journal --title="Field Notes"
guidedog ssg new posts/2026-04-17-zero-copy.rst --title="Zero-Copy Readers"
guidedog ssg schedule posts/2026-04-17-zero-copy.rst --at=2026-05-01T09:00:00Z
guidedog ssg publish posts/2026-04-17-zero-copy.rst
guidedog ssg build site
```

标有 `:pdf: article` 的页面还会在生成 HTML 页面的同时生成具有印刷品质的独立 PDF。多项目网站通过 `[[site.mounts]]` 以声明方式挂载子项目（例如 `docs/site`、`docs/manual` 和 `docs/gds`）；`docs/api` 则作为独立的参考文档项目及手册 PDF 附录构建。参见[静态站点发布](docs/manual/howto/static-publishing.rst)。

## 阅读与嵌入

- [手册](docs/manual/index.rst)：教程、专题、操作指南、参考资料和内部实现。
- [API 参考](docs/api/index.rst)：由 Guidedog 自身从 Rust 源码生成，并收录于手册的 PDF 附录中。运行 `cargo doc --workspace --no-deps` 也可以构建 rustdoc 页面。
- [引擎指南](crates/guidedoc/README.md)：工作区、请求、适配器和处理趟（pass）；[示例](examples/README.md)是可直接用 `cargo run` 运行的小程序。
- [模板](docs/manual/templates.rst)：用 Jinja 定制 HTML，用 Typst 定制图书。
- [GDS](docs/gds/README.md)：带编号的设计记录和实现决策。
- [讨论库](crates/gd-discussion/README.md)：`guidedog dsc` 所维护的目录。
- [插件库](crates/gd-plugins/README.md)：带类型的构建服务，以及供 `guidedog publish` 和链接进来的 Pages 插件使用的发布器接口。
- [可复用的 crate](docs/manual/internals/crates.rst)：面向独立使用而组织的引擎、库、API 生成器和插件 SDK（当前从此工作区构建，crate 名称已为 crates.io 预留），以及它们之间的依赖关系。

`crates/` 下的每个 crate 都有一份简短的 README，介绍其 API 和设计决策。Guidedoc 持有并在多次转换间复用自身的文档表，进入稳态后转换过程零内存分配。它会返回带提示信息的结构化诊断。宿主负责管理文件、资源预算、缓存、发布和内嵌编译器。`--untrusted` 仅限制能力，并非进程沙箱，也不限制所有原生资源开销。Beta 版的适用范围是 Linux、macOS 和 Windows 上的可信文档项目。

## 致谢与许可

由 **Vikrant Rathore** 创建，**Ronak Rathore** 和 **Kanak Rathore** 协助开发。欢迎通过 issue 和 pull request 贡献代码或文档。文档署名使用姓名或账号名，不公开个人邮箱；Git 身份设置单独管理。

感谢 Sphinx 和 Georg Brandl、Docutils 和 David Goodger、Typst、Steinberg Media Technologies 和 Daniel Spreadbury（Bravura 与 SMuFL）、Tree-sitter 和 Max Brunsfeld、Graphviz，以及 Rust 项目及其社区。

Guidedog 基于 [GNU 宽通用公共许可证 3.0 或更高版本](LICENSE)（LGPL-3.0-or-later）发布，该许可证在 [GNU 通用公共许可证 3.0](COPYING) 的基础上附加了额外许可。如需其他许可条款，请联系作者。[NOTICE](NOTICE) 说明了联系方式，并列出了保留各自许可证的第三方组件（如各 Rust crate 和内置字体，包括采用 SIL Open Font License 1.1 的 Bravura 字体）。附加许可允许将 Guidedog 与 Graphviz 链接并一同分发，详见 [NOTICE](NOTICE)。
