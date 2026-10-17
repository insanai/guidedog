# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**एक बार लिखें। साफ़ समझाएँ। वेब और काग़ज़ पर प्रकाशित करें।**

Guidedog उन्हीं reStructuredText या Markdown फ़ाइलों से दस्तावेज़ वेबसाइट और PDF पुस्तक बनाता है। हम स्रोत फ़ाइलों का एक ही सेट रखते हैं और उसे वेब तथा काग़ज़ के पाठकों के लिए प्रकाशित करते हैं। रिलीज़ में एक `guidedog` एक्ज़ीक्यूटेबल मिलता है, जिसमें PDF टाइपसेटिंग और आरेख रेंडरिंग शामिल है।

लेखन कार्यप्रवाह Sphinx से प्रेरित है। Guidedog एक स्वतंत्र प्रणाली है: कॉन्फ़िगरेशन घोषणात्मक TOML में होता है, API निष्कर्षण स्थिर (static) है, और Python कॉन्फ़िगरेशन या एक्सटेंशन कभी नहीं चलाए जाते। Sphinx से पूर्ण अनुकूलता लक्ष्य नहीं है।

## प्रोजेक्ट शुरू करें

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

प्रोजेक्ट में `conf.toml`, `index.rst`, `_templates/layout.html`, `_templates/book.typ` और `_static/` होते हैं। टेम्पलेट सामान्य फ़ाइलें हैं जिन्हें हम सीधे संपादित कर सकते हैं। HTML में नेविगेशन, खोज, कोड हाइलाइटिंग, आरेख और समीकरण मिलते हैं। PDF पुस्तकों में अध्याय, क्रॉस-रेफ़रेंस, चित्र, अनुक्रमणिका (इंडेक्स) और भाषा-सचेत टाइपसेटिंग मिलती है।

डिफ़ॉल्ट थीम हल्की (light) है। पाठक गहरी (dark) या सिस्टम थीम चुन सकते हैं। मैनुअल अपने मुख्य पृष्ठ और रिपॉज़िटरी परिचय पर ब्राउज़र की भाषा पहचान लेता है। सहेजी गई पसंद को प्राथमिकता मिलती है; किसी संस्करण या अध्याय के सीधे लिंक स्पष्ट अनुरोध ही माने जाते हैं।

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API दस्तावेज़ स्रोत कोड को चलाए बिना सीधे उससे पढ़ा जाता है: Python (`autodoc`), Odin (`odindoc`), और Rust क्रेट (`rust` डोमेन और `rust_autoapi_*` सेटिंग्स)।

## इंस्टॉल करें

हर [रिलीज़](https://github.com/insanai/guidedog/releases) में Linux (x86-64 और ARM64) और macOS (Apple silicon और Intel) के लिए आर्काइव तथा Windows x86-64 के लिए एक ZIP फ़ाइल होती है। हर आर्काइव में `guidedog` एक्ज़ीक्यूटेबल और उसकी सूचनाएँ (notices) होती हैं। GitHub Pages प्रकाशन, स्थिर साइट प्रकाशन और चर्चाएँ लिंक किए गए प्लगइन हैं; `gd-graphdog` Graphviz को एक लाइब्रेरी के रूप में समेटता है। Typst कंपाइलर अंतर्निहित (embedded) है, tree-sitter साथ में कंपाइल किया गया है, और Graphviz स्थिर रूप से (statically) लिंक है, इसलिए अलग से कुछ भी इंस्टॉल करने की ज़रूरत नहीं होती। Windows ZIP में Graphviz DLL शामिल होते हैं; निकाले गए पूरे फ़ोल्डर को एक साथ रखें। Linux के लिए glibc 2.35 या नया संस्करण चाहिए; macOS के लिए संस्करण 12 या नया चाहिए। हम अपने सिस्टम का आर्काइव और `SHA256SUMS` डाउनलोड करते हैं, फिर जाँच कर इंस्टॉल करते हैं:

```sh
sha256sum -c --ignore-missing SHA256SUMS      # on macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog /usr/local/bin/
```

macOS पर आधिकारिक [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog) से इंस्टॉल करें। Homebrew अपने आप Apple silicon या Intel आर्काइव चुन लेता है। यह tap Linux का भी समर्थन करता है:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

एक्ज़ीक्यूटेबल कोड-हस्ताक्षरित नहीं है: macOS पर ब्राउज़र से डाउनलोड की गई फ़ाइल का क्वारंटीन `xattr -d com.apple.quarantine guidedog` से हटाएँ। रिलीज़ नोट्स में हर प्लेटफ़ॉर्म की सीमाएँ बताई गई हैं।

## सोर्स से बिल्ड करें

Rust 1.99 या नया संस्करण ([rustup](https://rustup.rs) से), एक C कंपाइलर (tree-sitter के व्याकरण C में हैं), और Graphviz की डेवलपमेंट फ़ाइलें इंस्टॉल करें: Debian और Ubuntu पर `libgraphviz-dev`, Fedora पर `graphviz-devel`, macOS पर Homebrew से `graphviz`। (या रिलीज़ की तरह Graphviz को स्थिर रूप से बिल्ड करें: `cargo xtask graphviz`, फिर `GUIDEDOG_GRAPHVIZ_STATIC=target/graphviz/16.1.0` के साथ बिल्ड करें; तब रनटाइम पर Graphviz की ज़रूरत नहीं रहती।) फिर वर्कस्पेस बिल्ड करें:

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

एक्ज़ीक्यूटेबल `target/release` में मिलता है। Homebrew वाले macOS पर पहले लिंकर को Graphviz का पथ दें: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`। Typst कंपाइलर डिफ़ॉल्ट रूप से अंतर्निहित है (`gd-typst` का `embedded` फ़ीचर); इसके बिना Guidedog PDF के लिए इंस्टॉल किया गया `typst` 0.15.1 चलाता है। Linux पर अंतर्निहित कंपाइलर का पैकेज डाउनलोडर साथ में रखे (vendored) OpenSSL को लिंक करता है, जिसे बिल्ड करने के लिए `perl` और `make` चाहिए।

कोई बदलाव भेजने से पहले वही चलाएँ जो CI चलाता है:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # source limits: function and file length, line width
```

[RELEASING.md](RELEASING.md) रिलीज़ की कार्यप्रणाली बताता है।

## Guidedog से अनुवाद करें

```sh
mkdir -p docs/manual/_editions
guidedog build -b gettext docs/manual docs/manual/locales/templates -W
guidedog intl update -p docs/manual/locales/templates \
    -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog build -b html docs/manual docs/manual/_editions/de \
    -D language=de -D 'html_extra_path=[]' -W
```

PO फ़ाइलों में `msgstr` मानों का अनुवाद करें और उनकी समीक्षा करें। Guidedog POT फ़ाइलें निकालता है, कैटलॉग मिलाता है, प्रगति बताता है और हर संस्करण बिल्ड करता है। यह अपने आप अनुवाद नहीं गढ़ता। कमांड, पथ, कोड, API नाम और संदर्भ लक्ष्य अपरिवर्तित रहते हैं। केवल मैनुअल और रिपॉज़िटरी परिचय का अनुवाद होता है; API और GDS अंग्रेज़ी में रहते हैं। `cargo xtask build-site` सभी संस्करणों, उनकी PDF पुस्तकों और चर्चाओं को जोड़कर वेबसाइट तैयार करता है।

GitHub के पाठक ऊपर दिए गए लिंक से README की भाषा चुनते हैं। भाषा का स्वतः चयन जनरेट की गई दस्तावेज़ वेबसाइट पर होता है।

## GitHub Pages पर प्रकाशित करें

`guidedog publish pages` अंतर्निहित GitHub Pages प्लगइन के माध्यम से प्रकाशित करता है। यह Guidedog के अपने इंजन से बिल्ड करता है, अधूरे सेटअप को पूरा करने में हमारा मार्गदर्शन करता है, और एक बार में पूरी तैयार साइट प्रकाशित करता है। `guidedog pages` कॉन्फ़िगरेशन, क्रेडेंशियल, रिकवरी और वर्कफ़्लो जनरेशन उपलब्ध कराता है। हमें कोई अलग प्रकाशक (publisher) एक्ज़ीक्यूटेबल इंस्टॉल नहीं करना पड़ता।

```sh
guidedog publish pages --repository=octo/manual --project=docs
guidedog pages status
guidedog pages workflow --write
```

स्क्रिप्ट से सेटअप करने के लिए हम `guidedog pages init`, `auth import`, `bind` और `setup` का उपयोग करते हैं। जनरेट किए गए वर्कफ़्लो की मदद से GitHub Actions हर push के बाद साइट बिल्ड और डिप्लॉय कर सकता है। [GitHub Pages पर प्रकाशित करें](docs/manual/howto/github-pages.rst) देखें।

बिल्ड प्लगइन एक टाइप्ड, वर्शन-युक्त अनुबंध (contract) के माध्यम से स्रोत पाठ, पूरे HTML पेज और स्थिर ऐसेट बदलते हैं। प्लगइन एक साधारण Rust क्रेट होता है। हम इसे `gd_cli::run_with` के ज़रिए अपने `guidedog` में लिंक करते हैं, या अलग प्रोग्राम के रूप में चलाते हैं जिसे `guidedog plugin register` SHA-256 द्वारा पिन करता है। प्लगइन स्वयं `guidedog` में नए कमांड भी जोड़ सकता है, जो `guidedog --help` में दिखते हैं। `guidedog publish --timeout=SECONDS` किसी प्रकाशक के साथ हर पूरे आदान-प्रदान की समय-सीमा तय करता है (डिफ़ॉल्ट: 300 सेकंड); इसे `--wait` से अधिक रखें। यदि परिणाम अज्ञात रहे, तो दोबारा प्रकाशित करने से पहले रिकवरी चलाएँ।

## चर्चाएँ सँभालें

`guidedog dsc` प्रस्तावों और निर्णयों को reStructuredText रिकॉर्ड के रूप में रखता है—हर रिकॉर्ड के लिए एक फ़ाइल—और उनसे एक वेबसाइट तथा PDF पुस्तकें बनाता है। हर कैटलॉग का एक उपसर्ग (prefix) होता है: Guidedog के डिज़ाइन रिकॉर्ड `gds` का उपयोग करते हैं, और कोई दूसरा प्रोजेक्ट `eds` का उपयोग कर सकता है।

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

कैटलॉग की `discussion.toml` फ़ाइल उसकी जीवनचक्र स्थितियाँ, रिकॉर्ड प्रकार और क्रमांकन तय करती है, और `guidedog dsc config` उसे प्रिंट करता है। ये कमांड चर्चा प्लगइन से आते हैं, जो हर `guidedog` में लिंक रहता है, इसलिए कुछ और इंस्टॉल करने की ज़रूरत नहीं है। Guidedog का अपना `docs/gds` भी ऐसा ही एक कैटलॉग है: हम उस फ़ोल्डर में `guidedog dsc check` चलाते हैं। [चर्चा कैटलॉग रखना](docs/manual/howto/discussions.rst) देखें।

`gd-graphdog` वह लाइब्रेरी है जो Graphviz को समेटती है। Guidedog इसका उपयोग प्रोसेस के भीतर ही दस्तावेज़ के आरेख रेंडर करने के लिए करता है; यह कोई अलग CLI नहीं है।

## स्थिर साइटें प्रकाशित करें

`guidedog ssg` एक ही `conf.toml` और दस्तावेज़ वृक्ष (document tree) से लेख, शोध लॉग, संग्रह (collections), फ़ीड, पिन किए गए थीम आर्काइव और बहु-प्रोजेक्ट वेबसाइटें सँभालता है:

```sh
guidedog ssg init site --preset=journal --title="Field Notes"
guidedog ssg new posts/2026-04-17-zero-copy.rst --title="Zero-Copy Readers"
guidedog ssg schedule posts/2026-04-17-zero-copy.rst --at=2026-05-01T09:00:00Z
guidedog ssg publish posts/2026-04-17-zero-copy.rst
guidedog ssg build site
```

`:pdf: article` से चिह्नित पेज अपने HTML पेजों के साथ-साथ स्वतंत्र, प्रिंट-गुणवत्ता वाली PDF भी बनाते हैं। बहु-प्रोजेक्ट वेबसाइटें `[[site.mounts]]` के माध्यम से उप-प्रोजेक्टों (जैसे `docs/manual`, `docs/api` और `docs/gds`) को घोषणात्मक रूप से माउंट करती हैं। [स्थिर साइट प्रकाशन](docs/manual/howto/static-publishing.rst) देखें।

## पढ़ें और एम्बेड करें

- [मैनुअल](docs/manual/index.rst): ट्यूटोरियल, विषय, व्यावहारिक कार्य, संदर्भ और आंतरिक संरचना।
- [API संदर्भ](docs/api/index.rst): Guidedog स्वयं इसे Rust सोर्स से जनरेट करता है और मैनुअल की PDF के परिशिष्ट में शामिल करता है। `cargo doc --workspace --no-deps` से rustdoc पेज भी बनते हैं।
- [इंजन गाइड](crates/guidedoc/README.md): वर्कस्पेस, अनुरोध, एडेप्टर और पास; [उदाहरण](examples/README.md) छोटे प्रोग्राम हैं जिन्हें हम `cargo run` से चला सकते हैं।
- [टेम्पलेट](docs/manual/templates.rst): Jinja से HTML और Typst से पुस्तकों को कस्टमाइज़ करें।
- [GDS](docs/gds/README.md): क्रमांकित डिज़ाइन रिकॉर्ड और कार्यान्वयन के निर्णय।
- [चर्चा लाइब्रेरी](crates/gd-discussion/README.md): वे कैटलॉग जिन्हें `guidedog dsc` सँभालता है।
- [प्लगइन लाइब्रेरी](crates/gd-plugins/README.md): टाइप्ड बिल्ड सेवाएँ और `guidedog publish` तथा लिंक किए गए Pages प्लगइन द्वारा उपयोग किया जाने वाला प्रकाशक (publisher) इंटरफ़ेस।
- [पुनः प्रयोज्य क्रेट](docs/manual/internals/crates.rst): इंजन, लाइब्रेरियाँ, API जनरेटर और प्लगइन SDK जिन्हें अन्य Rust प्रोजेक्ट crates.io से ले सकते हैं, और उनकी आपसी निर्भरताएँ।

`crates/` के अंतर्गत हर क्रेट में एक संक्षिप्त README है जिसमें उसका API और डिज़ाइन निर्णय दिए गए हैं। Guidedoc अपनी दस्तावेज़ तालिकाओं का स्वामी है और रूपांतरणों के बीच उनका पुनः उपयोग करता है, इसलिए स्थिर अवस्था में किसी रूपांतरण के दौरान नया मेमोरी आवंटन नहीं होता। यह संकेत (hint) के साथ संरचित निदान (diagnostics) लौटाता है। होस्ट फ़ाइलें, बजट, कैशिंग, प्रकाशन और अंतर्निहित कंपाइलर सँभालता है। `--untrusted` क्षमताओं को सीमित करता है; यह कोई प्रोसेस सैंडबॉक्स या सभी मूल संसाधनों के उपयोग की सीमा नहीं है। बीटा का दायरा Linux, macOS और Windows पर भरोसेमंद दस्तावेज़ प्रोजेक्ट हैं।

## श्रेय और लाइसेंस

**Vikrant Rathore** द्वारा निर्मित, **Ronak Rathore** और **Kanak Rathore** के सहयोग से। इश्यू और पुल रिक्वेस्ट के माध्यम से योगदान का स्वागत है। दस्तावेज़ के श्रेय में नाम या हैंडल का उपयोग होता है, निजी ईमेल पतों का नहीं; Git पहचान सेटिंग्स अलग से सँभाली जाती हैं।

Sphinx और Georg Brandl, Docutils और David Goodger, Typst, Tree-sitter और Max Brunsfeld, Graphviz, तथा Rust प्रोजेक्ट और उसके समुदाय का धन्यवाद।

Guidedog [GNU Lesser General Public License v3.0 या उसके बाद के संस्करण](LICENSE) (LGPL-3.0-or-later) के अंतर्गत जारी किया गया है, जो [GNU General Public License v3.0](COPYING) में अतिरिक्त अनुमतियाँ जोड़ता है। अन्य लाइसेंसिंग शर्तों के लिए लेखकों से संपर्क करें। [NOTICE](NOTICE) में उनसे संपर्क करने का तरीका और उन तृतीय-पक्ष घटकों की सूची दी गई है—जैसे Rust क्रेट और शामिल फ़ॉन्ट—जिनके अपने लाइसेंस बने रहते हैं। एक अतिरिक्त अनुमति Guidedog को Graphviz के साथ लिंक और वितरित करने की छूट देती है; [NOTICE](NOTICE) देखें।
