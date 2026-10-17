# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**写一份源码，讲清楚道理，发布到网页与纸上。**

Guidedog 从同一组 reStructuredText 或 Markdown 文件生成文档网站和 PDF 图书。我们只需维护一份源文件，就能面向网页和纸面读者发布。发行版提供一个 `guidedog` 可执行文件，内含 PDF 排版和图表渲染功能。

Sphinx 启发了它的写作流程。Guidedog 是独立系统：配置使用声明式 TOML，API 提取采用静态分析，绝不执行 Python 配置或扩展。完全兼容 Sphinx 不是目标。

## 创建项目

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

项目包含 `conf.toml`、`index.rst`、`_templates/layout.html`、`_templates/book.typ` 和 `_static/`。模板是普通文件，我们可以直接编辑。HTML 提供导航、搜索、代码高亮、图表和公式。PDF 图书提供章节、交叉引用、插图、索引及按语言优化的排版。

默认采用浅色主题，读者也可选择深色或跟随系统。手册首页和仓库概览会检测浏览器语言。已保存的选择优先；直接访问某个版本或章节的链接则视为明确请求。

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API 文档直接从源码读取，不运行源码：Python（`autodoc`）、Odin（`odindoc`）以及 Rust crate（`rust` 域和 `rust_autoapi_*` 设置）。

## 安装

每个[发行版](https://github.com/insanai/guidedog/releases)都提供 Linux（x86-64 与 ARM64）和 macOS（Apple 芯片与 Intel）归档包，以及 Windows x86-64 ZIP 包。每个归档包都包含 `guidedog` 可执行文件及声明文件。GitHub Pages 发布、静态站点发布和讨论记录均为链接进来的插件；`gd-graphdog` 将 Graphviz 封装为库。Typst 编译器已内嵌，tree-sitter 已编译进程序，Graphviz 采用静态链接，因此无需额外部署其他依赖。Windows ZIP 包附带了 Graphviz DLL，解压后请保持整个文件夹完整。Linux 需要 glibc 2.35 或更高版本；macOS 需要 12 或更高版本。下载对应系统的归档包和 `SHA256SUMS`，校验后即可安装：

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

生成的可执行文件位于 `target/release`。在使用 Homebrew 的 macOS 上，先让链接器找到 Graphviz：`export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`。Typst 编译器默认内嵌（即 `gd-typst` 的 `embedded` 特性）；若不启用该特性，Guidedog 会调用系统中安装的 `typst` 0.15.1 生成 PDF。在 Linux 上，内嵌编译器的包下载器会链接随源码附带的 OpenSSL，构建它需要 `perl` 和 `make`。

提交修改前，请运行与 CI 相同的检查：

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # 源码限制：函数与文件长度、行宽
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

在 PO 文件中翻译并审阅 `msgstr` 的值。Guidedog 负责提取 POT 文件、合并消息目录、报告进度并构建各语言版本，不会自行编造译文。命令、路径、代码、API 名称和引用目标保持不变。仅翻译手册和仓库概览，API 与 GDS 保持英文。`cargo xtask build-site` 会将所有语言版本、对应的 PDF 图书和讨论记录汇总组装成网站。

GitHub 上的读者可通过上方的链接选择 README 语言。自动语言检测由生成的文档网站提供。

## 发布到 GitHub Pages

`guidedog publish pages` 通过内置的 GitHub Pages 插件进行发布。它使用 Guidedog 自身的引擎构建文档，引导我们完成缺失的配置，并发布一次完整的站点生成结果。`guidedog pages` 提供配置、凭据、恢复和工作流生成子命令。无需安装独立的发布程序。

```sh
guidedog publish pages --repository=octo/manual --project=docs
guidedog pages status
guidedog pages workflow --write
```

在脚本化配置中，我们可以使用 `guidedog pages init`、`auth import`、`bind` 和 `setup`。借助生成的工作流，GitHub Actions 可以在每次推送后自动构建并部署。参见[发布到 GitHub Pages](docs/manual/howto/github-pages.rst)。

构建插件通过带类型、带版本的契约转换源文本、完整 HTML 页面和静态资源。插件是普通的 Rust crate。我们可以用 `gd_cli::run_with` 将其链接进自己的 `guidedog`，也可以将其作为独立程序运行，并由 `guidedog plugin register` 按 SHA-256 固定。插件还可以向 `guidedog` 本身添加命令，并显示在 `guidedog --help` 中。`guidedog publish --timeout=SECONDS` 限制与发布器的每次完整交互时间（默认 300 秒）；请设置比 `--wait` 更长的值。当发布结果不明时，请先执行恢复，再重新发布。

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

目录的 `discussion.toml` 定义其生命周期状态、记录类型和编号规则，`guidedog dsc config` 可将其打印出来。这些命令来自讨论插件，每个 `guidedog` 都已链接该插件，因此无需安装任何额外组件。Guidedog 自身的 `docs/gds` 就是这样一个目录：我们可以在该目录下运行 `guidedog dsc check`。参见[维护讨论目录](docs/manual/howto/discussions.rst)。

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

标有 `:pdf: article` 的页面还会在生成 HTML 页面的同时生成具有印刷品质的独立 PDF。多项目网站通过 `[[site.mounts]]` 以声明方式挂载子项目（例如 `docs/manual`、`docs/api` 和 `docs/gds`）。参见[静态站点发布](docs/manual/howto/static-publishing.rst)。

## 阅读与嵌入

- [手册](docs/manual/index.rst)：教程、专题、操作指南、参考资料和内部实现。
- [API 参考](docs/api/index.rst)：由 Guidedog 自身从 Rust 源码生成，并收录于手册的 PDF 附录中。运行 `cargo doc --workspace --no-deps` 也可以构建 rustdoc 页面。
- [引擎指南](crates/guidedoc/README.md)：工作区、请求、适配器和处理趟（pass）；[示例](examples/README.md)是可直接用 `cargo run` 运行的小程序。
- [模板](docs/manual/templates.rst)：用 Jinja 定制 HTML，用 Typst 定制图书。
- [GDS](docs/gds/README.md)：带编号的设计记录和实现决策。
- [讨论库](crates/gd-discussion/README.md)：`guidedog dsc` 所维护的目录。
- [插件库](crates/gd-plugins/README.md)：带类型的构建服务，以及供 `guidedog publish` 和链接进来的 Pages 插件使用的发布器接口。
- [可复用的 crate](docs/manual/internals/crates.rst)：供其他 Rust 项目从 crates.io 引入的引擎、库、API 生成器和插件 SDK，以及它们之间的依赖关系。

`crates/` 下的每个 crate 都有一份简短的 README，介绍其 API 和设计决策。Guidedoc 持有自己的文档表并在多次转换间复用，因此在稳定状态下的转换实现零内存分配。它会返回带提示信息的结构化诊断。宿主负责管理文件、资源预算、缓存、发布和内嵌编译器。`--untrusted` 仅限制能力，并非进程沙箱，也不限制所有原生资源开销。Beta 版的适用范围是 Linux、macOS 和 Windows 上的可信文档项目。

## 致谢与许可

由 **Vikrant Rathore** 创建，**Ronak Rathore** 和 **Kanak Rathore** 协助开发。欢迎通过 issue 和 pull request 贡献代码或文档。文档署名使用姓名或账号名，不公开个人邮箱；Git 身份设置单独管理。

感谢 Sphinx 和 Georg Brandl、Docutils 和 David Goodger、Typst、Tree-sitter 和 Max Brunsfeld、Graphviz，以及 Rust 项目及其社区。

Guidedog 基于 [GNU 宽通用公共许可证 3.0 或更高版本](LICENSE)（LGPL-3.0-or-later）发布，该许可证在 [GNU 通用公共许可证 3.0](COPYING) 的基础上附加了额外许可。如需其他许可条款，请联系作者。[NOTICE](NOTICE) 说明了联系方式，并列出了保留各自许可证的第三方组件（如各 Rust crate 和内置字体）。附加许可允许将 Guidedog 与 Graphviz 链接并一同分发，详见 [NOTICE](NOTICE)。
