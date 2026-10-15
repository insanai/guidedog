# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**写一份源码，讲清楚道理，发布到网页与纸上。**

Guidedog 将 reStructuredText 和 Markdown 构建为 HTML 文档网站和 PDF 图书。它使用 Rust 编写。转换引擎名为 **Guidedoc**，PDF 由内置于程序中的 Typst 排版。无需 Python 环境或 LaTeX 安装。

Sphinx 启发了它的写作流程。Guidedog 是独立系统：配置使用声明式 TOML，API 提取采用静态分析，绝不执行 Python 配置或扩展。完全兼容 Sphinx 不是目标。

## 创建项目

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

项目包含 `conf.toml`、`index.rst`、`_templates/layout.html`、`_templates/book.typ` 和 `_static/`。模板是普通文件，我们可以直接编辑。HTML 提供导航、搜索、代码高亮、图表和公式。PDF 提供章节、交叉引用、插图、索引及按语言优化的排版。

默认采用浅色主题，也可选择深色或跟随系统。手册首页和仓库概览会按浏览器语言选择版本。已保存的选择优先；直接进入某个语言版本或章节时，会保留该链接的选择。

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API 文档直接从源码读取，不运行源码：Python（`autodoc`）、Odin（`odindoc`），以及 0.7 新增的 Rust crate（`rust` 域和 `rust_autoapi_*` 设置）。

## 安装

发行包仅包含一个可执行程序：`guidedog`。内置插件提供 `guidedog dsc`、`guidedog publish pages` 和 `guidedog pages` 命令。`gd-graphdog` 是封装 Graphviz 的库。Linux 和 macOS 支持 x86-64 与 ARM64，Windows 支持 x86-64。Windows ZIP 解压后应保留整个目录及其 DLL。

```sh
sha256sum -c --ignore-missing SHA256SUMS      # macOS 上：shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog \
    /usr/local/bin/
```

在 macOS 上，通过官方 [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog) 安装。Homebrew 会自动选择 Apple 芯片或 Intel 芯片对应的安装包，并一同安装 Graphviz。该 tap 也支持 Linux：

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

这些程序没有代码签名：在 macOS 上，用 `xattr -d com.apple.quarantine guidedog` 解除浏览器下载文件的隔离属性。各平台的限制见版本说明。

## 从源码构建

安装 Rust 1.99 或更新版本（使用 [rustup](https://rustup.rs)）、C 编译器（tree-sitter 的语法以 C 编写），以及 Graphviz 开发文件：Debian 和 Ubuntu 上为 `libgraphviz-dev`，Fedora 上为 `graphviz-devel`，macOS 上为 Homebrew 的 `graphviz`。然后构建工作区：

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

程序位于 `target/release`。在使用 Homebrew 的 macOS 上，先让链接器找到 Graphviz：`export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`。Typst 编译器默认内置（`gd-typst` 的 `embedded` 特性）；不启用该特性时，Guidedog 使用已安装的 `typst` 0.15.1 生成 PDF。在 Linux 上，内置编译器的包下载器会链接随源码提供的 OpenSSL，构建它需要 `perl` 和 `make`。

提交修改之前，运行 CI 所运行的检查：

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # 源码限制：函数和文件长度、行宽
```

[RELEASING.md](RELEASING.md) 介绍发布流程。

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

在 PO 文件中填写并审阅 `msgstr`。Guidedog 负责提取 POT、合并目录、报告进度和构建各语言版本，不会自动编写翻译。命令、路径、代码、API 名称和引用目标保持不变。只翻译手册和仓库概览，API 与 GDS 保持英文。`cargo xtask build-site` 把所有语言版本、它们的 PDF 图书和讨论记录汇总成网站。

GitHub 上的读者用上方链接选择 README 语言。自动语言选择由生成的文档网站提供。

## 发布到 GitHub Pages

`guidedog publish pages` 从我们自己的机器发布网站。它用已安装的 `guidedog` 构建项目，以一次 Git 推送把整个网站发送到专用分支。只有当分支仍是读取时的那个提交，GitHub 才会接受这次推送。之后它一直等到 GitHub Pages 部署完该提交。令牌保存在系统的凭据存储中，本机上的绑定决定一个凭据档案可以发布到哪里。

```sh
guidedog pages init --repository=octo/manual --project=docs
guidedog pages auth import          # 细粒度令牌，读取时不回显
guidedog pages bind                 # 核对发布目标，然后允许本机发布
guidedog publish pages
guidedog pages setup                # 让 GitHub Pages 从该分支部署
guidedog pages watch                # 源文件每次变化后重新发布
```

另一种方式是 GitHub Actions，适合在每次推送后发布。`guidedog publish --publisher=pages` 用 Guidedog 自己的引擎构建，并运行同一个发布器。见[发布到 GitHub Pages](docs/manual/howto/github-pages.rst)和[编写插件](docs/manual/plugins.rst)。

构建插件通过有明确类型、带版本的契约转换源文本、完整 HTML 页面和静态资源。插件是普通的 Rust crate。我们可以用 `gd_cli::run_with` 把它链接进我们自己的 `guidedog`，也可以把它作为独立程序运行，由 `guidedog plugin register` 按 SHA-256 固定。`guidedog publish --timeout=SECONDS` 限制与发布器的每次完整交互，默认 300 秒，应大于 `--wait`。结果不明时，先恢复，再重新发布。

## 维护讨论

`guidedog dsc` 把提案和决策保存为 reStructuredText 记录，每条一个文件，并构建成网站和 PDF 图书。每个目录都有前缀：Guidedog 的设计记录使用 `gds`，其他项目可以使用 `eds`。

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

目录的 `discussion.toml` 设定它的生命周期状态、记录类型和编号，`guidedog dsc config` 会打印它。这些命令来自讨论插件，每个 `guidedog` 都链接了该插件，因此无需另外安装任何东西。Guidedog 自己的 `docs/gds` 就是这样一个目录：我们在该文件夹中运行 `guidedog dsc check`。见[维护讨论目录](docs/manual/howto/discussions.rst)。


## 阅读与嵌入

- [手册](docs/manual/index.rst)：教程、专题、操作指南、参考和内部原理。
- [API 参考](docs/api/index.rst)：由 Guidedog 自己从 Rust 源码生成，并作为附录收入手册 PDF。`cargo doc --workspace --no-deps` 也可以构建 rustdoc 页面。
- [引擎指南](crates/guidedoc/README.md)：工作区、请求、适配器和处理步骤（pass）；[示例](examples/README.md)是我们可以用 `cargo run` 运行的小程序。
- [模板](docs/manual/templates.rst)：用 Jinja 自定义 HTML，用 Typst 自定义图书。
- [GDS](docs/gds/README.md)：编号的设计记录和实现决策。
- [讨论库](crates/gd-discussion/README.md)：`guidedog dsc` 维护的目录。
- [插件库](crates/gd-plugins/README.md)：带类型的构建服务，以及 `guidedog publish` 和 `guidedog publish pages` 使用的发布器接口。
- [可复用的 crate](docs/manual/internals/crates.rst)：其他 Rust 项目可以从 crates.io 使用的引擎、库、API 生成器和插件 SDK，以及它们之间的依赖关系。

`crates/` 下的每个 crate 都有一份简短的 README，说明它的 API 和设计决策。Guidedoc 拥有自己的文档表，并在多次转换之间复用它们，因此稳定状态下的转换不分配内存。它返回带提示的结构化诊断。宿主管理文件、预算、缓存、发布和内置编译器。`--untrusted` 只限制能力，不是进程沙箱，也不限制全部原生资源消耗。Beta 面向 macOS 和 Linux 上的可信文档项目。

## 致谢与许可

由 **Vikrant Rathore** 创建，**Ronak Rathore** 和 **Kanak Rathore** 协助开发。欢迎通过 issue 和 pull request 参与。文档署名使用姓名或账号名，不公开个人邮箱；Git 身份设置单独管理。

感谢 Sphinx 和 Georg Brandl、Docutils 和 David Goodger、Typst、Tree-sitter 和 Max Brunsfeld、Graphviz、Rust 项目及其社区，以及 Ginger Bill 和 Odin 社区。

Guidedog 以 [GNU 宽通用公共许可证 3.0 或更高版本](LICENSE)（LGPL-3.0-or-later）发布，该许可证在 [GNU 通用公共许可证 3.0](COPYING) 的基础上增加了额外许可。如需其他许可条款，请联系作者。[NOTICE](NOTICE) 说明了联系方式，并列出保留各自许可证的第三方组件，例如 Rust crate 和捆绑字体。附加许可允许将 Guidedog 与 Graphviz 链接并一同分发，详见 [NOTICE](NOTICE)。
