# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**한 번 쓰고, 명확히 설명하고, 웹과 종이로 전하세요.**

Guidedog은 reStructuredText와 Markdown으로 HTML 문서 사이트와 PDF 책을 만듭니다. Rust로 구현되었으며 변환 엔진의 이름은 **Guidedoc**입니다. PDF는 프로그램에 내장된 Typst로 조판합니다. Python 환경이나 LaTeX 설치가 필요하지 않습니다.

작성 흐름은 Sphinx에서 배웠지만 Guidedog은 독립적인 시스템입니다. 설정은 선언형 TOML이며 API는 정적으로 추출합니다. Python 설정이나 확장은 실행하지 않습니다. Sphinx와의 완전한 호환은 목표가 아닙니다.

## 프로젝트 시작

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

프로젝트에는 `conf.toml`, `index.rst`, `_templates/layout.html`, `_templates/book.typ`, `_static/` 이 있습니다. 템플릿은 우리가 직접 편집할 수 있는 일반 파일입니다. HTML은 탐색, 검색, 코드 강조, 도표, 수식을 제공합니다. PDF는 장, 상호 참조, 그림, 색인, 언어별 조판을 제공합니다.

기본 테마는 밝게이며 어둡게 또는 시스템 설정을 선택할 수 있습니다. 매뉴얼 첫 페이지와 저장소 개요는 브라우저 언어에 맞는 판을 선택합니다. 저장한 선택이 우선하며 특정 언어판이나 장으로 직접 들어가는 링크는 그대로 따릅니다.

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API 문서는 소스를 실행하지 않고 읽습니다. Python(`autodoc`), Odin(`odindoc`), 그리고 0.7에서 새로 추가된 Rust 크레이트(`rust` 도메인과 `rust_autoapi_*` 설정)를 지원합니다.

## 설치

각 [릴리스](https://github.com/insanai/guidedog/releases)에는 Linux(x86-64, ARM64)와 macOS(Apple 실리콘, Intel)용 압축 파일이 있습니다. `guidedog` 압축 파일에는 `guidedog`, `guidedog-pages`, `graphdog` 가 들어 있습니다. Typst 컴파일러는 내장되어 있고 tree-sitter도 프로그램에 컴파일되어 있습니다. Graphviz는 시스템의 것을 링크하므로 먼저 그 라이브러리를 설치하세요. Debian과 Ubuntu에서는 `sudo apt install libgvc6`(또는 `graphviz` 패키지), Fedora에서는 `sudo dnf install graphviz`, macOS에서는 `brew install graphviz` 입니다. Linux에는 glibc 2.35 이상이, macOS에는 버전 12 이상이 필요합니다. 우리 시스템에 맞는 압축 파일과 `SHA256SUMS` 를 내려받아 확인한 뒤 설치합니다.

```sh
sha256sum -c --ignore-missing SHA256SUMS      # macOS에서는: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog \
    guidedog-linux-amd64/guidedog-pages guidedog-linux-amd64/graphdog /usr/local/bin/
```

macOS에서는 공식 [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog)에서 설치하세요. Homebrew가 Apple Silicon 또는 Intel용 패키지를 자동으로 선택하고 Graphviz도 함께 설치합니다. 이 tap은 Linux도 지원합니다.

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

프로그램에는 코드 서명이 없습니다. macOS에서 브라우저로 내려받았다면 `xattr -d com.apple.quarantine guidedog` 으로 격리 속성을 제거하세요. 플랫폼별 제한 사항은 릴리스 노트에 있습니다.

## 소스에서 빌드

Rust 1.90 이상([rustup](https://rustup.rs)으로 설치), C 컴파일러(tree-sitter의 문법은 C로 작성되어 있습니다), Graphviz 개발 파일을 설치하세요. Debian과 Ubuntu에서는 `libgraphviz-dev`, Fedora에서는 `graphviz-devel`, macOS에서는 Homebrew의 `graphviz` 입니다. 그런 다음 워크스페이스를 빌드합니다.

```sh
cargo build --release -p guidedog-cmd -p guidedog-pages-cmd -p graphdog-cmd
target/release/guidedog --version
```

프로그램은 `target/release` 에 생깁니다. Homebrew를 쓰는 macOS에서는 먼저 링커가 Graphviz를 찾도록 하세요: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`. Typst 컴파일러는 기본으로 내장됩니다(`gd-typst` 의 `embedded` 기능). 이 기능을 끄면 Guidedog은 PDF에 설치된 `typst` 0.15.1을 사용합니다. Linux에서는 내장 컴파일러의 패키지 다운로더가 함께 제공되는 OpenSSL을 링크하므로, 이를 빌드하려면 `perl` 과 `make` 가 필요합니다.

변경 사항을 보내기 전에 CI가 실행하는 것을 실행하세요.

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # 소스 제한: 함수와 파일 길이, 줄 너비
```

릴리스 절차는 [RELEASING.md](RELEASING.md)에 설명되어 있습니다.

## Guidedog으로 번역

```sh
mkdir -p docs/manual/_editions
guidedog build -b gettext docs/manual docs/manual/locales/templates -W
guidedog intl update -p docs/manual/locales/templates \
    -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog build -b html docs/manual docs/manual/_editions/de \
    -D language=de -D 'html_extra_path=[]' -W
```

PO 파일의 `msgstr` 을 번역하고 검토하세요. Guidedog은 POT 추출, 카탈로그 병합, 진행 상황 표시, 각 언어판 빌드를 처리하지만 번역문을 만들어 주지는 않습니다. 명령, 경로, 코드, API 이름, 참조 대상은 바꾸지 않습니다. 매뉴얼과 저장소 개요만 번역하며 API와 GDS는 영어로 유지합니다. `cargo xtask build-site` 는 모든 언어판과 그 PDF 책, 논의 기록을 웹사이트로 조합합니다.

GitHub에서는 위 링크로 README 언어를 선택합니다. 자동 언어 선택은 생성된 문서 웹사이트에서 동작합니다.

## GitHub Pages에 게시

`guidedog-pages` 는 우리 컴퓨터에서 사이트를 게시합니다. 설치된 `guidedog` 으로 프로젝트를 빌드하고, 사이트 전체를 Git 푸시 한 번으로 사이트 전용 브랜치에 보냅니다. GitHub는 브랜치가 읽었던 커밋을 그대로 가리키고 있을 때만 이 푸시를 반영합니다. 그런 다음 GitHub Pages가 그 커밋을 배포할 때까지 기다립니다. 토큰은 시스템의 자격 증명 저장소에 보관하며, 프로필이 어디에 게시할 수 있는지는 컴퓨터에 기록한 연결(binding)이 정합니다.

```sh
guidedog-pages init --repository=octo/manual --project=docs
guidedog-pages auth import          # 세분화된 토큰을 화면에 표시하지 않고 입력받습니다
guidedog-pages bind                 # 대상을 확인한 뒤 이 컴퓨터의 게시를 허용합니다
guidedog-pages publish
guidedog-pages setup                # GitHub Pages가 이 브랜치에서 배포하도록 설정합니다
guidedog-pages watch                # 소스가 바뀔 때마다 다시 게시합니다
```

푸시할 때마다 게시하려면 다른 방법인 GitHub Actions를 사용하세요. `guidedog publish --publisher=pages` 는 Guidedog 자체 엔진으로 빌드하고 같은 게시기를 실행합니다. [GitHub Pages에 게시하기](docs/manual/howto/github-pages.rst)와 [플러그인 작성하기](docs/manual/plugins.rst)를 참고하세요.

빌드 플러그인은 타입이 명확하고 버전이 매겨진 계약을 통해 원문, 전체 HTML 페이지, 정적 리소스를 변환합니다. 플러그인은 일반 Rust 크레이트입니다. 우리는 `gd_cli::run_with` 로 우리의 `guidedog` 에 링크하거나, `guidedog plugin register` 가 SHA-256으로 고정하는 별도 프로그램으로 실행할 수 있습니다. `guidedog publish --timeout=SECONDS` 는 게시기와의 교환 전체의 기한입니다. 기본값은 300초이며 `--wait` 보다 길게 잡으세요. 결과가 불확실하면 다시 게시하기 전에 복구하세요.

## 논의 기록 관리

`guidedog dsc` 는 제안과 결정을 파일 하나에 하나씩 reStructuredText 기록으로 보관하고, 이를 웹사이트와 PDF 책으로 만듭니다. 카탈로그마다 접두사가 있습니다. Guidedog의 설계 기록은 `gds` 를 쓰고, 다른 프로젝트는 `eds` 를 쓸 수 있습니다.

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

카탈로그의 `discussion.toml` 은 수명 주기의 상태, 기록 유형, 번호 매기기를 정하고, `guidedog dsc config` 는 이를 출력합니다. 이 명령들은 모든 `guidedog` 에 링크된 논의 플러그인이 제공하므로 따로 설치할 것이 없습니다. Guidedog 자체의 `docs/gds` 도 그런 카탈로그 중 하나로, 그 폴더에서 `guidedog dsc check` 를 실행합니다. [논의 카탈로그 관리하기](docs/manual/howto/discussions.rst)를 참고하세요.

`graphdog` 는 Guidedog의 도표와 같은 라이브러리로 Graphviz DOT를 SVG, PNG 또는 레이아웃이 적용된 DOT로 렌더링하며, `dot` 명령의 일부 기능을 제공합니다: `echo 'digraph { a -> b }' | graphdog -Tsvg -o graph.svg`.

## 문서 읽기와 임베딩

- [매뉴얼](docs/manual/index.rst): 튜토리얼, 주제 설명, 작업 안내, 참조, 내부 구조.
- [API 참조](docs/api/index.rst): Guidedog이 직접 Rust 소스에서 생성하며 매뉴얼 PDF의 부록에도 포함합니다. `cargo doc --workspace --no-deps` 로 rustdoc 페이지도 빌드할 수 있습니다.
- [엔진 안내](crates/guidedoc/README.md): 작업 공간, 요청, 어댑터, 패스. [예제](examples/README.md)는 우리가 `cargo run` 으로 실행할 수 있는 작은 프로그램입니다.
- [템플릿](docs/manual/templates.rst): Jinja로 HTML을, Typst로 책을 사용자화합니다.
- [GDS](docs/gds/README.md): 번호가 지정된 설계 기록과 구현 결정.
- [논의 라이브러리](crates/gd-discussion/README.md): `guidedog dsc` 가 관리하는 카탈로그.
- [플러그인 라이브러리](crates/gd-plugins/README.md): 타입이 명확한 빌드 서비스와 `guidedog publish` 및 `guidedog-pages` 가 사용하는 게시기 인터페이스.
- [재사용 가능한 크레이트](docs/manual/internals/crates.rst): 다른 Rust 프로젝트가 crates.io 에서 가져다 쓸 수 있는 엔진, 라이브러리, API 생성기, 플러그인 SDK 와 그 의존 관계.

`crates/` 아래의 각 크레이트에는 API와 이식 과정의 설계 결정을 담은 짧은 README가 있습니다. Guidedoc은 문서 테이블을 직접 소유하고 변환 사이에 재사용하므로, 안정 상태의 변환은 메모리를 할당하지 않습니다. 또한 힌트가 포함된 구조화된 진단을 반환합니다. 호스트는 파일, 예산, 캐시, 게시, 내장 컴파일러를 관리합니다. `--untrusted` 는 기능을 제한할 뿐 프로세스 샌드박스나 모든 네이티브 자원의 상한은 아닙니다. 베타는 macOS와 Linux의 신뢰할 수 있는 문서 프로젝트를 대상으로 합니다.

Guidedog 0.6 이하는 Odin으로 작성되었습니다. 버전 0.7은 Rust로 이식한 것으로 명령, 옵션, `conf.toml` 키, 지시문, 역할, 출력을 그대로 유지합니다. 바뀐 점은 [변경 기록](CHANGELOG.md)에 있습니다.

## 기여자와 라이선스

**Vikrant Rathore**가 만들고 **Ronak Rathore**와 **Kanak Rathore**가 개발을 도왔습니다. 이슈와 풀 리퀘스트를 통한 기여를 환영합니다. 문서에는 이름이나 계정 이름만 표기하고 개인 이메일 주소는 공개하지 않습니다. Git 작성자 설정은 별도로 관리합니다.

Sphinx와 Georg Brandl, Docutils와 David Goodger, Typst, Tree-sitter와 Max Brunsfeld, Graphviz, Rust 프로젝트와 그 커뮤니티, 그리고 Guidedog의 첫 구현을 담은 언어를 만든 Ginger Bill과 Odin 커뮤니티에 감사드립니다.

Guidedog은 [GNU Affero General Public License v3.0 이상](LICENSE)(AGPL-3.0-or-later)으로 배포합니다. 다른 라이선스 조건이 필요하면 저자에게 문의하세요. 연락 방법과, Rust 크레이트와 포함된 글꼴처럼 각자의 라이선스를 따르는 서드파티 구성 요소 목록은 [NOTICE](NOTICE)에 있습니다. 추가 허가에 따라 Guidedog을 Graphviz와 링크하여 배포할 수 있습니다. 자세한 내용은 [NOTICE](NOTICE)를 참고하세요.
