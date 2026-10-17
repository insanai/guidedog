# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**Escriba una vez. Explique con claridad. Publique en la web y en papel.**

Guidedog crea un sitio de documentación y un libro PDF a partir de los mismos archivos reStructuredText o Markdown. Mantenemos una sola copia del contenido y la publicamos para la web y el papel. La distribución incluye un único ejecutable, `guidedog`, con composición de PDF y generación de diagramas.

El flujo de escritura se inspira en Sphinx, pero Guidedog es un sistema independiente. Usa TOML declarativo y extracción estática de API. Nunca ejecuta configuración ni extensiones Python. No pretende ser completamente compatible con Sphinx.

## Crear un proyecto

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

El proyecto contiene `conf.toml`, `index.rst`, `_templates/layout.html`, `_templates/book.typ` y `_static/`. Las plantillas son archivos corrientes que podemos editar. HTML ofrece navegación, búsqueda, código resaltado, diagramas y ecuaciones. PDF ofrece capítulos, referencias, figuras, índices y composición adaptada al idioma.

El tema predeterminado es claro; también puede elegir oscuro o el del sistema. La portada del manual y el resumen del repositorio detectan el idioma del navegador. La elección guardada tiene prioridad y se respetan los enlaces directos a una edición o capítulo.

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

La documentación de API se lee del código fuente sin ejecutarlo: Python (`autodoc`), Odin (`odindoc`) y crates de Rust (el dominio `rust` y los ajustes `rust_autoapi_*`).

## Instalar

Cada [versión](https://github.com/insanai/guidedog/releases) ofrece un archivo para Linux (x86-64 y ARM64) y macOS (Apple silicon e Intel), además de un ZIP para Windows x86-64. Cada archivo contiene el ejecutable `guidedog` y sus avisos legales. La publicación en GitHub Pages, la publicación de sitios estáticos y las discusiones son plugins enlazados; `gd-graphdog` encapsula Graphviz como biblioteca. El compilador de Typst va integrado, tree-sitter está compilado dentro del binario y Graphviz se enlaza estáticamente, por lo que no hace falta instalar nada más. El ZIP de Windows incluye las DLL de Graphviz; conservamos junta toda la carpeta extraída. Linux necesita glibc 2.35 o posterior; macOS necesita la versión 12 o posterior. Descargamos el archivo para nuestro sistema y `SHA256SUMS`, lo comprobamos y lo instalamos:

```sh
sha256sum -c --ignore-missing SHA256SUMS      # en macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog /usr/local/bin/
```

En macOS, instale desde el [tap oficial de Guidedog para Homebrew](https://github.com/insanai/homebrew-guidedog). Homebrew selecciona el archivo para Apple silicon o Intel. El tap también admite Linux:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

El ejecutable no está firmado: en macOS, quite la cuarentena de una descarga del navegador con `xattr -d com.apple.quarantine guidedog`. Las notas de la versión describen los límites de cada plataforma.

## Compilar desde las fuentes

Instale Rust 1.99 o posterior (con [rustup](https://rustup.rs)), un compilador de C (las gramáticas de tree-sitter están escritas en C) y los archivos de desarrollo de Graphviz: `libgraphviz-dev` en Debian y Ubuntu, `graphviz-devel` en Fedora, `graphviz` de Homebrew en macOS. (O compile Graphviz estáticamente, como hacen las versiones: `cargo xtask graphviz`, y después compile con `GUIDEDOG_GRAPHVIZ_STATIC=target/graphviz/16.1.0`; así los programas no necesitan Graphviz en tiempo de ejecución). Después compile el espacio de trabajo:

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

El ejecutable queda en `target/release`. En macOS con Homebrew, indique primero al enlazador dónde está Graphviz: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`. El compilador de Typst va integrado por defecto (la característica `embedded` de `gd-typst`); sin ella, Guidedog ejecuta un `typst` 0.15.1 instalado para PDF. En Linux, el descargador de paquetes del compilador integrado enlaza un OpenSSL incluido en el código, que necesita `perl` y `make` para compilarse.

Antes de enviar un cambio, ejecute lo mismo que la CI:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # límites del código: longitud de funciones y archivos, ancho de línea
```

[RELEASING.md](RELEASING.md) describe el proceso de publicación.

## Traducir con Guidedog

```sh
mkdir -p docs/manual/_editions
guidedog build -b gettext docs/manual docs/manual/locales/templates -W
guidedog intl update -p docs/manual/locales/templates \
    -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog build -b html docs/manual docs/manual/_editions/de \
    -D language=de -D 'html_extra_path=[]' -W
```

Traduzca y revise los valores `msgstr` de los archivos PO. Guidedog extrae archivos POT, fusiona catálogos, informa del progreso y compila cada edición; no inventa traducciones. Los comandos, las rutas, el código, los nombres de API y los destinos de referencia no se modifican. Solo se traducen el manual y el resumen del repositorio; la API y los GDS siguen en inglés. `cargo xtask build-site` reúne todas las ediciones, sus libros PDF y las discusiones en el sitio web.

En GitHub, los lectores eligen el idioma del README con los enlaces de arriba. La selección automática de idioma forma parte del sitio web de documentación generado.

## Publicar en GitHub Pages

`guidedog publish pages` publica mediante el plugin integrado de GitHub Pages. Compila con el motor propio de Guidedog, nos guía por la configuración que falte y publica una generación completa del sitio. `guidedog pages` expone la configuración, las credenciales, la recuperación y la generación del flujo de trabajo. No instalamos ningún ejecutable publicador aparte.

```sh
guidedog publish pages --repository=octo/manual --project=docs
guidedog pages status
guidedog pages workflow --write
```

Para una configuración automatizada mediante scripts usamos `guidedog pages init`, `auth import`, `bind` y `setup`. GitHub Actions puede compilar y desplegar tras cada push mediante el flujo de trabajo generado. Consulte [Publicar en GitHub Pages](docs/manual/howto/github-pages.rst).

Los plugins de compilación transforman texto fuente, páginas HTML completas y recursos estáticos mediante un contrato tipado y versionado. Un plugin es un crate de Rust corriente. Lo enlazamos en nuestro propio `guidedog` con `gd_cli::run_with`, o lo ejecutamos como programa aparte que `guidedog plugin register` fija por su SHA-256. Un plugin también puede añadir comandos al propio `guidedog`, que aparecen en `guidedog --help`. `guidedog publish --timeout=SECONDS` limita cada intercambio completo con un publicador (300 segundos por defecto); elija un valor mayor que `--wait`. Si el resultado queda incierto, recupérelo antes de volver a publicar.

## Mantener discusiones

`guidedog dsc` guarda propuestas y decisiones como registros reStructuredText, un archivo por registro, y los convierte en un sitio web y libros PDF. Cada catálogo tiene un prefijo: los registros de diseño de Guidedog usan `gds`, y otro proyecto podría usar `eds`.

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

El `discussion.toml` de un catálogo fija los estados de su ciclo de vida, sus tipos de registro y su numeración, y `guidedog dsc config` lo muestra. Los comandos vienen del plugin de discusiones, que todo `guidedog` lleva enlazado, así que no hace falta instalar nada más. El propio `docs/gds` de Guidedog es uno de esos catálogos: ejecutamos `guidedog dsc check` en esa carpeta. Consulte [Mantener un catálogo de discusiones](docs/manual/howto/discussions.rst).

`gd-graphdog` es la biblioteca que encapsula Graphviz. Guidedog la usa para generar diagramas de documentación en el mismo proceso; no es una CLI independiente.

## Publicar sitios estáticos

`guidedog ssg` gestiona artículos, cuadernos de investigación, colecciones, canales de sindicación, archivos de temas fijados y sitios web multiproyecto desde el mismo `conf.toml` y árbol de documentos:

```sh
guidedog ssg init site --preset=journal --title="Field Notes"
guidedog ssg new posts/2026-04-17-zero-copy.rst --title="Zero-Copy Readers"
guidedog ssg schedule posts/2026-04-17-zero-copy.rst --at=2026-05-01T09:00:00Z
guidedog ssg publish posts/2026-04-17-zero-copy.rst
guidedog ssg build site
```

Las páginas marcadas con `:pdf: article` también generan archivos PDF independientes con calidad de impresión junto a sus páginas HTML. Los sitios web multiproyecto montan subproyectos (como `docs/manual`, `docs/api` y `docs/gds`) de forma declarativa mediante `[[site.mounts]]`. Consulte [Publicación de sitios estáticos](docs/manual/howto/static-publishing.rst).

## Leer e integrar

- [Manual](docs/manual/index.rst): tutoriales, temas, tareas prácticas, referencia y funcionamiento interno.
- [Referencia de la API](docs/api/index.rst): generada por el propio Guidedog a partir del código Rust e incluida como apéndice del PDF del manual. `cargo doc --workspace --no-deps` también genera las páginas de rustdoc.
- [Guía del motor](crates/guidedoc/README.md): espacios de trabajo, solicitudes, adaptadores y pasadas; los [ejemplos](examples/README.md) son programas pequeños que podemos ejecutar con `cargo run`.
- [Plantillas](docs/manual/templates.rst): personalizar HTML con Jinja y libros con Typst.
- [GDS](docs/gds/README.md): registros numerados de diseño y decisiones de implementación.
- [Biblioteca de discusiones](crates/gd-discussion/README.md): los catálogos que mantiene `guidedog dsc`.
- [Biblioteca de plugins](crates/gd-plugins/README.md): servicios de compilación tipados y la interfaz de publicadores que usan `guidedog publish` y el plugin enlazado de Pages.
- [Crates reutilizables](docs/manual/internals/crates.rst): el motor, las bibliotecas, los generadores de API y el SDK de plugins que otros proyectos Rust pueden tomar de crates.io, y cómo dependen unos de otros.

Cada crate de `crates/` tiene un README breve con su API y sus decisiones de diseño. Guidedoc posee sus tablas de documentos y las reutiliza entre conversiones, de modo que una conversión en régimen estable no reserva memoria. Devuelve diagnósticos estructurados con una sugerencia. El anfitrión gestiona archivos, presupuestos, caché, publicación y los compiladores integrados. `--untrusted` restringe capacidades; no es un aislamiento de procesos ni un límite sobre todo el uso de recursos nativos. El alcance de la beta son proyectos de documentación de confianza en Linux, macOS y Windows.

## Créditos y licencia

Creado por **Vikrant Rathore**, con la colaboración de **Ronak Rathore** y **Kanak Rathore**. Se agradecen las contribuciones mediante issues y pull requests. Los créditos de la documentación usan nombres o identificadores, no direcciones personales de correo; la configuración de identidad de Git se gestiona por separado.

Gracias a Sphinx y Georg Brandl, Docutils y David Goodger, Typst, Tree-sitter y Max Brunsfeld, Graphviz y el proyecto Rust y su comunidad.

Guidedog se distribuye bajo la [Licencia Pública General Reducida de GNU v3.0 o posterior](LICENSE) (LGPL-3.0-or-later), que añade permisos a la [Licencia Pública General de GNU v3.0](COPYING). Para otras condiciones de licencia, contacte con los autores. [NOTICE](NOTICE) explica cómo hacerlo y enumera los componentes de terceros, como los crates de Rust y las fuentes incluidas, que conservan sus propias licencias. Un permiso adicional autoriza enlazar y distribuir Guidedog junto con Graphviz; consulte [NOTICE](NOTICE).
