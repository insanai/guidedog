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

La documentación de API se lee del código fuente sin ejecutarlo: Python (`autodoc`), Odin (`odindoc`) y, como novedad de la 0.7, crates de Rust (el dominio `rust` y los ajustes `rust_autoapi_*`).

## Instalar

Los archivos de la versión contienen un solo ejecutable: `guidedog`. Sus plugins añaden `guidedog dsc`, `guidedog publish pages` y `guidedog pages`. `gd-graphdog` encapsula Graphviz como biblioteca. Linux y macOS admiten x86-64 y ARM64; Windows admite x86-64. Conservamos toda la carpeta del ZIP de Windows, incluidas sus DLL.

```sh
sha256sum -c --ignore-missing SHA256SUMS      # en macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog \
    /usr/local/bin/
```

En macOS, instale desde el [tap oficial de Guidedog para Homebrew](https://github.com/insanai/homebrew-guidedog). Homebrew elige el archivo para Apple silicon o Intel e instala Graphviz con él. El tap también admite Linux:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

Los programas no están firmados: en macOS, quite la cuarentena de una descarga del navegador con `xattr -d com.apple.quarantine guidedog`. Las notas de la versión describen los límites de cada plataforma.

## Compilar desde las fuentes

Instale Rust 1.99 o posterior (con [rustup](https://rustup.rs)), un compilador de C (las gramáticas de tree-sitter están escritas en C) y los archivos de desarrollo de Graphviz: `libgraphviz-dev` en Debian y Ubuntu, `graphviz-devel` en Fedora, `graphviz` de Homebrew en macOS. Después compile el espacio de trabajo:

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

Los programas quedan en `target/release`. En macOS con Homebrew, indique primero al enlazador dónde está Graphviz: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`. El compilador de Typst va integrado por defecto (la característica `embedded` de `gd-typst`); sin ella, Guidedog ejecuta un `typst` 0.15.1 instalado para PDF. En Linux, el descargador de paquetes del compilador integrado enlaza un OpenSSL incluido en el código, que necesita `perl` y `make` para compilarse.

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

Traduzca y revise los valores `msgstr` de los archivos PO. Guidedog extrae POT, fusiona catálogos, informa del progreso y compila cada edición; no redacta traducciones. Mantenga comandos, rutas, código, nombres de API y destinos de referencia. Solo se traducen el manual y el resumen del repositorio; API y GDS siguen en inglés. `cargo xtask build-site` reúne todas las ediciones, sus libros PDF y las discusiones en el sitio web.

En GitHub, los lectores eligen el idioma del README con los enlaces de arriba. La selección automática funciona en el sitio generado.

## Publicar en GitHub Pages

`guidedog publish pages` publica un sitio desde nuestra propia máquina. Compila el proyecto con el `guidedog` instalado y envía el sitio completo a una rama propia en un solo push de Git. GitHub aplica el push solo si la rama conserva el commit que se leyó. Después espera hasta que GitHub Pages haya desplegado ese commit. Los tokens se quedan en el almacén de credenciales del sistema, y una vinculación en nuestra máquina decide dónde puede publicar un perfil.

```sh
guidedog pages init --repository=octo/manual --project=docs
guidedog pages auth import          # un token de permisos detallados, leído sin mostrarlo
guidedog pages bind                 # revise el destino y autorice esta máquina
guidedog publish pages
guidedog pages setup                # desplegar GitHub Pages desde la rama
guidedog pages watch                # volver a publicar cada vez que cambien las fuentes
```

GitHub Actions es la otra vía, para publicar después de cada push. `guidedog publish --publisher=pages` compila con el motor propio de Guidedog y ejecuta el mismo publicador. Consulte [Publicar en GitHub Pages](docs/manual/howto/github-pages.rst) y [Escribir un plugin](docs/manual/plugins.rst).

Los plugins de compilación transforman texto fuente, páginas HTML completas y recursos estáticos mediante un contrato tipado y versionado. Un plugin es un crate de Rust corriente. Lo enlazamos en nuestro propio `guidedog` con `gd_cli::run_with`, o lo ejecutamos como programa aparte que `guidedog plugin register` fija por su SHA-256. `guidedog publish --timeout=SECONDS` limita cada intercambio completo con un publicador (300 segundos por defecto); elija un valor mayor que `--wait`. Si el resultado es incierto, recupérelo antes de volver a publicar.

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


## Leer e integrar

- [Manual](docs/manual/index.rst): tutoriales, temas, tareas, referencia y funcionamiento interno.
- [Referencia API](docs/api/index.rst): generada por el propio Guidedog a partir del código Rust e incluida como apéndice del PDF del manual. `cargo doc --workspace --no-deps` también genera las páginas de rustdoc.
- [Guía del motor](crates/guidedoc/README.md): espacios de trabajo, solicitudes, adaptadores y pases; los [ejemplos](examples/README.md) son programas pequeños que ejecutamos con `cargo run`.
- [Plantillas](docs/manual/templates.rst): personalizar HTML con Jinja y libros con Typst.
- [GDS](docs/gds/README.md): registros numerados de diseño y decisiones de implementación.
- [Biblioteca de discusiones](crates/gd-discussion/README.md): los catálogos que mantiene `guidedog dsc`.
- [Biblioteca de plugins](crates/gd-plugins/README.md): servicios de compilación con tipos definidos y la interfaz de publicadores que usan `guidedog publish` y `guidedog publish pages`.
- [Crates reutilizables](docs/manual/internals/crates.rst): el motor, las bibliotecas, los generadores de API y el SDK de plugins que otros proyectos Rust pueden tomar de crates.io, y cómo dependen unos de otros.

Cada crate de `crates/` tiene un README breve con su API y sus decisiones de diseño. Guidedoc posee sus tablas de documento y las reutiliza entre conversiones, de modo que una conversión en régimen estable no reserva memoria. Guidedoc devuelve diagnósticos estructurados con una sugerencia. El gestor se ocupa de archivos, presupuestos, caché, publicación y los compiladores integrados. `--untrusted` limita capacidades; no aísla procesos ni limita todos los recursos nativos. La beta está dirigida a proyectos de confianza en macOS y Linux.

## Créditos y licencia

Creado por **Vikrant Rathore**, con la colaboración de **Ronak Rathore** y **Kanak Rathore**. Se agradecen las contribuciones mediante issues y pull requests. Los créditos usan nombres o identificadores, sin direcciones personales de correo; la identidad de Git se gestiona por separado.

Gracias a Sphinx y Georg Brandl, Docutils y David Goodger, Typst, Tree-sitter y Max Brunsfeld, Graphviz y el proyecto Rust y su comunidad.

Guidedog se distribuye bajo la [Licencia Pública General Reducida de GNU v3.0 o posterior](LICENSE) (LGPL-3.0-or-later), que añade permisos a la [Licencia Pública General de GNU v3.0](COPYING). Para otras condiciones de licencia, contacte con los autores. [NOTICE](NOTICE) explica cómo hacerlo y enumera los componentes de terceros que conservan sus propias licencias, como los crates de Rust y las fuentes incluidas. Un permiso adicional autoriza enlazar y distribuir Guidedog junto con Graphviz; consulte [NOTICE](NOTICE).
