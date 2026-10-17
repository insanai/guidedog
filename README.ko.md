# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**한 번 쓰고, 명확히 설명하고, 웹과 종이로 펴내세요.**

Guidedog은 같은 reStructuredText 또는 Markdown 파일로 문서 웹사이트와 PDF 책을 만듭니다. 하나의 원본 파일을 관리하며 웹과 종이의 독자에게 전합니다. 릴리스는 PDF 조판과 다이어그램 렌더링을 포함한 실행 파일 `guidedog` 하나로 제공됩니다.

작성 흐름은 Sphinx에서 영감을 받았지만 Guidedog은 독립적인 시스템입니다. 설정은 선언형 TOML이며, API는 정적으로 추출하고, Python 설정이나 확장은 실행하지 않습니다. Sphinx와의 완전한 호환은 목표가 아닙니다.

## 프로젝트 시작

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

프로젝트에는 `conf.toml`, `index.rst`, `_templates/layout.html`, `_templates/book.typ`, `_static/`이 들어 있습니다. 템플릿은 직접 편집할 수 있는 일반 파일입니다. HTML은 탐색, 검색, 코드 강조, 다이어그램, 수식을 제공합니다. PDF 책은 장, 상호 참조, 그림, 색인, 언어에 맞춘 조판을 제공합니다.

기본 테마는 밝은 테마이며, 독자는 어두운 테마나 시스템 테마를 선택할 수 있습니다. 매뉴얼 첫 페이지와 저장소 개요는 브라우저 언어를 감지합니다. 저장된 선택이 우선하며, 특정 언어판이나 장으로 바로 연결되는 링크는 명시적 요청으로 그대로 따릅니다.

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API 문서는 소스를 실행하지 않고 정적으로 읽어 들입니다. Python(`autodoc`), Odin(`odindoc`), Rust 크레이트(`rust` 도메인과 `rust_autoapi_*` 설정)를 지원합니다.

## 설치

각 [릴리스](https://github.com/insanai/guidedog/releases)는 Linux(x86-64 및 ARM64)와 macOS(Apple Silicon 및 Intel)용 압축 파일, 그리고 Windows x86-64용 ZIP을 제공합니다. 각 압축 파일에는 `guidedog` 실행 파일과 고지 사항이 들어 있습니다. GitHub Pages 게시, 정적 사이트 게시, 논의 기록 관리는 링크된 내장 플러그인으로 제공되며, `gd-graphdog`는 Graphviz를 라이브러리로 캡슐화합니다. Typst 컴파일러가 내장되어 있고, tree-sitter가 함께 컴파일되어 있으며, Graphviz가 정적으로 링크되어 있으므로 별도로 설치할 것이 없습니다. Windows ZIP에는 Graphviz DLL이 함께 들어 있으므로 압축을 푼 폴더 전체를 한곳에 유지하세요. Linux는 glibc 2.35 이상, macOS는 버전 12 이상이 필요합니다. 시스템에 맞는 압축 파일과 `SHA256SUMS`를 내려받은 뒤 체크섬을 확인하고 설치합니다:

```sh
sha256sum -c --ignore-missing SHA256SUMS      # macOS에서는: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog /usr/local/bin/
```

macOS에서는 공식 [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog)으로 설치할 수 있습니다. Homebrew가 Apple Silicon 또는 Intel용 압축 파일을 자동으로 선택합니다. 이 tap은 Linux도 지원합니다:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

실행 파일에는 코드 서명이 되어 있지 않습니다. macOS에서 브라우저로 내려받았다면 `xattr -d com.apple.quarantine guidedog` 명령으로 격리 속성을 제거하세요. 플랫폼별 제한 사항은 릴리스 노트에 설명되어 있습니다.

## 소스에서 빌드

Rust 1.99 이상([rustup](https://rustup.rs)으로 설치), C 컴파일러(tree-sitter 문법이 C로 작성되어 있음), Graphviz 개발 파일을 설치하세요. Debian과 Ubuntu에서는 `libgraphviz-dev`, Fedora에서는 `graphviz-devel`, macOS에서는 Homebrew의 `graphviz`입니다. (또는 공식 릴리스처럼 Graphviz를 정적으로 빌드할 수도 있습니다. `cargo xtask graphviz`를 실행한 뒤 `GUIDEDOG_GRAPHVIZ_STATIC=target/graphviz/16.1.0`을 지정해 빌드하면 런타임에 Graphviz가 필요 없습니다.) 그런 다음 워크스페이스를 빌드합니다:

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

실행 파일은 `target/release`에 생성됩니다. Homebrew를 사용하는 macOS에서는 먼저 링커가 Graphviz를 찾도록 `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`를 설정하세요. Typst 컴파일러는 기본으로 내장됩니다(`gd-typst`의 `embedded` 기능). 이 기능을 끄면 Guidedog은 PDF 생성을 위해 시스템에 설치된 `typst` 0.15.1을 실행합니다. Linux에서는 내장 컴파일러의 패키지 다운로더가 함께 번들된 OpenSSL을 링크하므로, 빌드할 때 `perl`과 `make`가 필요합니다.

변경 사항을 보내기 전에 CI가 실행하는 검사를 수행하세요:

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

PO 파일의 `msgstr` 값을 번역하고 검토하세요. Guidedog은 POT 파일을 추출하고, 카탈로그를 병합하며, 진행 상황을 보고하고, 각 언어판을 빌드하지만 번역을 대신 만들어 주지는 않습니다. 명령어, 경로, 코드, API 이름, 참조 대상은 그대로 유지됩니다. 매뉴얼과 저장소 개요만 번역하며, API와 GDS는 영어로 유지합니다. `cargo xtask build-site`는 모든 언어판과 각 언어판의 PDF 책, 논의 기록을 하나의 웹사이트로 조립합니다.

GitHub 독자는 상단 링크를 통해 README 언어를 선택합니다. 자동 언어 선택은 생성된 문서 웹사이트에서 동작합니다.

## GitHub Pages에 게시

`guidedog publish pages`는 내장된 GitHub Pages 플러그인을 통해 사이트를 게시합니다. Guidedog 자체 엔진으로 빌드하고, 누락된 설정이 있으면 안내하며, 완성된 사이트 세대 하나를 게시합니다. `guidedog pages`는 설정, 자격 증명, 복구, 워크플로 생성 기능을 제공합니다. 별도의 게시기 실행 파일을 설치할 필요가 없습니다.

```sh
guidedog publish pages --repository=octo/manual --project=docs
guidedog pages status
guidedog pages workflow --write
```

스크립트로 설정할 때는 `guidedog pages init`, `auth import`, `bind`, `setup`을 사용합니다. 생성된 워크플로를 사용하면 푸시할 때마다 GitHub Actions에서 빌드하고 배포할 수 있습니다. 자세한 내용은 [GitHub Pages에 게시하기](docs/manual/howto/github-pages.rst)를 참고하세요.

빌드 플러그인은 타입이 지정되고 버전이 관리되는 계약을 통해 원본 텍스트, 완성된 HTML 페이지, 정적 에셋을 변환합니다. 플러그인은 일반 Rust 크레이트입니다. `gd_cli::run_with`로 자체 `guidedog`에 링크하거나, `guidedog plugin register`가 SHA-256으로 고정하는 별도 프로그램으로 실행할 수 있습니다. 또한 플러그인은 `guidedog --help`에 표시되는 명령을 `guidedog` 자체에 추가할 수도 있습니다. `guidedog publish --timeout=SECONDS`는 게시기와의 전체 교환 제한 시간을 설정합니다(기본값: 300초). `--wait`보다 긴 값을 지정하세요. 결과를 알 수 없는 상태로 끝났다면 다시 게시하기 전에 복구를 수행하세요.

## 논의 기록 관리

`guidedog dsc`는 제안과 결정을 파일당 하나씩 reStructuredText 기록으로 보관하고, 이를 웹사이트와 PDF 책으로 빌드합니다. 각 카탈로그에는 접두사가 있습니다. Guidedog의 설계 기록은 `gds`를 사용하며, 다른 프로젝트는 예를 들어 `eds`를 사용할 수 있습니다.

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

카탈로그의 `discussion.toml`은 수명 주기 상태, 기록 유형, 번호 매기기 규칙을 설정하며, `guidedog dsc config`로 이를 출력할 수 있습니다. 이 명령들은 모든 `guidedog`에 링크된 논의 플러그인이 제공하므로 별도로 설치할 것이 없습니다. Guidedog 자체의 `docs/gds`도 이러한 카탈로그 중 하나이며, 해당 폴더에서 `guidedog dsc check`를 실행합니다. 자세한 내용은 [논의 카탈로그 관리하기](docs/manual/howto/discussions.rst)를 참고하세요.

`gd-graphdog`는 Graphviz를 캡슐화한 라이브러리입니다. Guidedog은 프로세스 내에서 문서 다이어그램을 렌더링할 때 이 라이브러리를 사용하며, 별도의 CLI는 아닙니다.

## 정적 사이트 게시

`guidedog ssg`는 동일한 `conf.toml`과 문서 트리에서 글(article), 연구 기록, 컬렉션, 피드, 해시로 고정된 테마 아카이브, 다중 프로젝트 웹사이트를 관리합니다:

```sh
guidedog ssg init site --preset=journal --title="Field Notes"
guidedog ssg new posts/2026-04-17-zero-copy.rst --title="Zero-Copy Readers"
guidedog ssg schedule posts/2026-04-17-zero-copy.rst --at=2026-05-01T09:00:00Z
guidedog ssg publish posts/2026-04-17-zero-copy.rst
guidedog ssg build site
```

`:pdf: article`이 표시된 페이지는 HTML 페이지와 함께 인쇄 품질의 독립 실행형 PDF로도 빌드됩니다. 다중 프로젝트 웹사이트는 `[[site.mounts]]`를 통해 하위 프로젝트(`docs/manual`, `docs/api`, `docs/gds` 등)를 선언적으로 마운트합니다. 자세한 내용은 [정적 사이트 게시](docs/manual/howto/static-publishing.rst)를 참고하세요.

## 문서 읽기와 임베딩

- [매뉴얼](docs/manual/index.rst): 튜토리얼, 주제 설명, 실전 가이드, 레퍼런스, 내부 구조.
- [API 레퍼런스](docs/api/index.rst): Guidedog 자체가 Rust 소스에서 직접 생성하며 매뉴얼 PDF의 부록에도 포함됩니다. `cargo doc --workspace --no-deps`로 rustdoc 페이지도 빌드할 수 있습니다.
- [엔진 가이드](crates/guidedoc/README.md): 워크스페이스, 요청, 어댑터, 패스를 다루며, [예제](examples/README.md)는 `cargo run`으로 실행해 볼 수 있는 작은 프로그램입니다.
- [템플릿](docs/manual/templates.rst): Jinja로 HTML을, Typst로 PDF 책을 사용자화합니다.
- [GDS](docs/gds/README.md): 번호가 부여된 설계 기록과 구현 결정.
- [논의 라이브러리](crates/gd-discussion/README.md): `guidedog dsc`가 관리하는 카탈로그 라이브러리.
- [플러그인 라이브러리](crates/gd-plugins/README.md): 타입이 지정된 빌드 서비스와 `guidedog publish` 및 링크된 Pages 플러그인이 사용하는 게시기 인터페이스.
- [재사용 가능한 크레이트](docs/manual/internals/crates.rst): 다른 Rust 프로젝트가 crates.io에서 가져다 쓸 수 있는 엔진, 라이브러리, API 생성기, 플러그인 SDK와 이들 사이의 의존 관계.

`crates/` 아래의 각 크레이트에는 API와 설계 결정을 요약한 짧은 README가 있습니다. Guidedoc은 문서 테이블을 직접 소유하고 변환 사이에 재사용하므로, 안정 상태(steady state)의 변환에서는 메모리를 새로 할당하지 않습니다. 또한 힌트가 포함된 구조화된 진단을 반환합니다. 호스트는 파일, 예산, 캐싱, 게시, 내장 컴파일러를 관리합니다. `--untrusted`는 기능을 제한할 뿐 프로세스 샌드박스나 모든 네이티브 자원 사용량에 대한 상한은 아닙니다. 베타 범위는 Linux, macOS, Windows의 신뢰할 수 있는 문서 프로젝트입니다.

## 기여자와 라이선스

**Vikrant Rathore**가 만들고 **Ronak Rathore**와 **Kanak Rathore**가 도왔습니다. 이슈와 풀 리퀘스트를 통한 기여를 환영합니다. 문서의 기여자 표기에는 개인 이메일 주소 대신 이름이나 핸들을 사용하며, Git 신원 설정은 별도로 관리합니다.

Sphinx와 Georg Brandl, Docutils와 David Goodger, Typst, Tree-sitter와 Max Brunsfeld, Graphviz, 그리고 Rust 프로젝트와 커뮤니티에 감사드립니다.

Guidedog은 [GNU General Public License v3.0](COPYING)에 추가 허가를 더한 [GNU Lesser General Public License v3.0 이상](LICENSE)(LGPL-3.0-or-later)에 따라 배포됩니다. 다른 라이선스 조건이 필요하면 저자에게 문의하세요. [NOTICE](NOTICE)에는 저자에게 연락하는 방법과, Rust 크레이트 및 번들 글꼴처럼 자체 라이선스를 유지하는 서드파티 구성 요소 목록이 나와 있습니다. 추가 허가에 따라 Guidedog을 Graphviz와 링크하여 배포할 수 있으며, 자세한 내용은 [NOTICE](NOTICE)를 참고하세요.
