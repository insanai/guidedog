# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**एक बार लिखें। साफ़ समझाएँ। वेब और कागज़ पर प्रकाशित करें।**

Guidedog उन्हीं reStructuredText या Markdown फ़ाइलों से दस्तावेज़ वेबसाइट और PDF पुस्तक बनाता है। हम सामग्री की एक प्रति बनाए रखते हैं और उसे वेब तथा कागज़ पर पढ़ने वालों के लिए प्रकाशित करते हैं। रिलीज़ में एक `guidedog` एक्ज़ीक्यूटेबल मिलता है, जिसमें PDF की पृष्ठ-सज्जा और आरेख बनाने की सुविधा शामिल है।

लेखन की प्रक्रिया Sphinx से प्रेरित है, पर Guidedog एक स्वतंत्र प्रणाली है। कॉन्फ़िगरेशन घोषणात्मक TOML में होता है और API का स्थिर विश्लेषण होता है। Python कॉन्फ़िगरेशन या एक्सटेंशन कभी नहीं चलते। Sphinx से पूरी अनुकूलता लक्ष्य नहीं है।

## परियोजना शुरू करें

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

परियोजना में `conf.toml`, `index.rst`, `_templates/layout.html`, `_templates/book.typ` और `_static/` होते हैं। टेम्पलेट सामान्य फ़ाइलें हैं जिन्हें हम सीधे बदल सकते हैं। HTML में नेविगेशन, खोज, कोड हाइलाइटिंग, आरेख और समीकरण मिलते हैं। PDF में अध्याय, पार-संदर्भ, चित्र, इंडेक्स और भाषा के अनुसार टाइपसेटिंग मिलती है।

डिफ़ॉल्ट थीम हल्की है; गहरी या सिस्टम थीम भी चुन सकते हैं। मैनुअल के मुख्य पृष्ठ और रिपॉज़िटरी परिचय पर ब्राउज़र की भाषा चुनी जाती है। सहेजी गई पसंद पहले लागू होती है। किसी भाषा या अध्याय के सीधे लिंक की पसंद नहीं बदली जाती।

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API दस्तावेज़ स्रोत कोड को चलाए बिना उससे पढ़ा जाता है: Python (`autodoc`), Odin (`odindoc`), और 0.7 में नया, Rust crates (`rust` डोमेन और `rust_autoapi_*` सेटिंग)।

## स्थापित करें

रिलीज़ में केवल एक प्रोग्राम है: `guidedog`। इसके प्लगइन `guidedog dsc`, `guidedog publish pages` और `guidedog pages` कमांड जोड़ते हैं। `gd-graphdog` Graphviz को एक लाइब्रेरी में समेटता है। Linux और macOS पर x86-64 और ARM64 उपलब्ध हैं; Windows पर x86-64। Windows ZIP की DLL फ़ाइलें उसी फ़ोल्डर में रखें।

```sh
sha256sum -c --ignore-missing SHA256SUMS      # macOS पर: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog \
    /usr/local/bin/
```

macOS पर आधिकारिक [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog) से स्थापित करें। Homebrew हमारे Apple silicon या Intel प्रोसेसर के लिए सही पैकेज चुनता है और उसके साथ Graphviz भी स्थापित करता है। यह tap Linux पर भी काम करता है:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

प्रोग्रामों पर कोड-हस्ताक्षर नहीं है: macOS पर ब्राउज़र से डाउनलोड की गई फ़ाइल का क्वारंटीन `xattr -d com.apple.quarantine guidedog` से हटाएँ। हर प्लेटफ़ॉर्म की सीमाएँ रिलीज़ नोट्स में लिखी हैं।

## स्रोत से बिल्ड करें

Rust 1.99 या नया ([rustup](https://rustup.rs) से), एक C कंपाइलर (tree-sitter के व्याकरण C में हैं), और Graphviz की डेवलपमेंट फ़ाइलें स्थापित करें: Debian और Ubuntu पर `libgraphviz-dev`, Fedora पर `graphviz-devel`, macOS पर Homebrew से `graphviz`। फिर workspace बिल्ड करें:

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

प्रोग्राम `target/release` में मिलते हैं। Homebrew वाले macOS पर पहले लिंकर को Graphviz का पता दें: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`। Typst कंपाइलर डिफ़ॉल्ट रूप से अंतर्निहित है (`gd-typst` का `embedded` फ़ीचर); इसके बिना Guidedog PDF के लिए स्थापित `typst` 0.15.1 चलाता है। Linux पर अंतर्निहित कंपाइलर का पैकेज डाउनलोडर साथ में दिया गया (vendored) OpenSSL लिंक करता है, जिसे बिल्ड करने के लिए `perl` और `make` चाहिए।

कोई बदलाव भेजने से पहले वही चलाएँ जो CI चलाता है:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # स्रोत की सीमाएँ: फ़ंक्शन और फ़ाइल की लंबाई, पंक्ति की चौड़ाई
```

[RELEASING.md](RELEASING.md) रिलीज़ की प्रक्रिया बताता है।

## Guidedog से अनुवाद

```sh
mkdir -p docs/manual/_editions
guidedog build -b gettext docs/manual docs/manual/locales/templates -W
guidedog intl update -p docs/manual/locales/templates \
    -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog build -b html docs/manual docs/manual/_editions/de \
    -D language=de -D 'html_extra_path=[]' -W
```

PO फ़ाइलों में `msgstr` का अनुवाद लिखें और जाँचें। Guidedog POT निकालता है, कैटलॉग मिलाता है, प्रगति बताता है और हर भाषा का संस्करण बनाता है; अनुवाद खुद नहीं लिखता। कमांड, पथ, कोड, API नाम और संदर्भ लक्ष्य न बदलें। केवल मैनुअल और रिपॉज़िटरी परिचय का अनुवाद होता है; API और GDS अंग्रेज़ी में रहते हैं। `cargo xtask build-site` सारे संस्करण, उनकी PDF पुस्तकें और चर्चाएँ जोड़कर वेबसाइट बनाता है।

GitHub पर पाठक ऊपर के लिंक से README की भाषा चुनते हैं। भाषा का स्वतः चयन बनी हुई दस्तावेज़ वेबसाइट पर होता है।

## GitHub Pages पर प्रकाशित करें

`guidedog publish pages` हमारी अपनी मशीन से साइट प्रकाशित करता है। यह स्थापित `guidedog` से परियोजना बिल्ड करता है और पूरी साइट को एक Git push में अपनी अलग ब्रांच पर भेजता है। GitHub यह push तभी लागू करता है जब ब्रांच पर अब भी वही कमिट हो जो पढ़ा गया था। फिर यह तब तक रुकता है जब तक GitHub Pages उस कमिट को डिप्लॉय न कर दे। टोकन सिस्टम के क्रेडेंशियल स्टोर में रहते हैं, और हमारी मशीन पर दर्ज बाइंडिंग तय करती है कि कोई प्रोफ़ाइल कहाँ प्रकाशित कर सकती है।

```sh
guidedog pages init --repository=octo/manual --project=docs
guidedog pages auth import          # fine-grained टोकन, जो टाइप करते समय दिखता नहीं
guidedog pages bind                 # लक्ष्य जाँचें, फिर इस मशीन को अनुमति दें
guidedog publish pages
guidedog pages setup                # GitHub Pages को इस ब्रांच से डिप्लॉय कराएँ
guidedog pages watch                # स्रोत बदलने पर हर बार फिर प्रकाशित करें
```

दूसरा रास्ता GitHub Actions है, जो हर push के बाद प्रकाशित करता है। `guidedog publish --publisher=pages` Guidedog के अपने इंजन से बिल्ड करता है और यही प्रकाशक (publisher) चलाता है। [GitHub Pages पर प्रकाशित करें](docs/manual/howto/github-pages.rst) और [Plugin लिखें](docs/manual/plugins.rst) देखें।

Build plugins एक typed, versioned अनुबंध के ज़रिए मूल पाठ, पूरे HTML पेज और स्थिर assets बदलते हैं। Plugin एक सामान्य Rust crate है। हम इसे `gd_cli::run_with` से अपने `guidedog` में लिंक करते हैं, या अलग प्रोग्राम के रूप में चलाते हैं जिसे `guidedog plugin register` उसके SHA-256 से बाँधता है। `guidedog publish --timeout=SECONDS` किसी publisher के साथ हर पूरे आदान-प्रदान की सीमा है (default: 300 सेकंड); इसे `--wait` से अधिक रखें। परिणाम अनिश्चित हो तो दोबारा publish करने से पहले recovery चलाएँ।

## चर्चाएँ सँभालें

`guidedog dsc` प्रस्तावों और निर्णयों को reStructuredText रिकॉर्ड के रूप में रखता है, हर एक की अलग फ़ाइल, और उनसे वेबसाइट और PDF पुस्तकें बनाता है। हर कैटलॉग का एक उपसर्ग होता है: Guidedog के डिज़ाइन रिकॉर्ड `gds` का उपयोग करते हैं, और कोई दूसरा प्रोजेक्ट `eds` का उपयोग कर सकता है।

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

कैटलॉग की `discussion.toml` फ़ाइल उसकी जीवनचक्र स्थितियाँ, रिकॉर्ड प्रकार और क्रमांकन तय करती है, और `guidedog dsc config` उसे प्रिंट करता है। ये कमांड चर्चा प्लगइन से आते हैं, जो हर `guidedog` में लिंक है, इसलिए और कुछ स्थापित करने की ज़रूरत नहीं है। Guidedog का अपना `docs/gds` भी ऐसा ही एक कैटलॉग है: हम उस फ़ोल्डर में `guidedog dsc check` चलाते हैं। [चर्चा कैटलॉग रखना](docs/manual/howto/discussions.rst) देखें।


## पढ़ें और एम्बेड करें

- [मैनुअल](docs/manual/index.rst): ट्यूटोरियल, विषय, व्यावहारिक काम, संदर्भ और आंतरिक संरचना।
- [API संदर्भ](docs/api/index.rst): Guidedog खुद इसे Rust स्रोत से बनाता है, और यह मैनुअल PDF के परिशिष्ट में भी रहता है। `cargo doc --workspace --no-deps` rustdoc पृष्ठ भी बनाता है।
- [इंजन गाइड](crates/guidedoc/README.md): कार्यक्षेत्र (workspace), अनुरोध, एडेप्टर और पास; [उदाहरण](examples/README.md) छोटे प्रोग्राम हैं जिन्हें हम `cargo run` से चला सकते हैं।
- [टेम्पलेट](docs/manual/templates.rst): Jinja से HTML और Typst से पुस्तक का रूप बदलें।
- [GDS](docs/gds/README.md): क्रमांकित डिज़ाइन अभिलेख और कार्यान्वयन के निर्णय।
- [चर्चा लाइब्रेरी](crates/gd-discussion/README.md): वे कैटलॉग जिन्हें `guidedog dsc` सँभालता है।
- [प्लगइन लाइब्रेरी](crates/gd-plugins/README.md): typed build services और `guidedog publish` तथा `guidedog publish pages` में इस्तेमाल होने वाला publisher इंटरफ़ेस।
- [पुन: उपयोग योग्य crates](docs/manual/internals/crates.rst): engine, libraries, API generators और plugin SDK, जिन्हें दूसरे Rust प्रोजेक्ट crates.io से ले सकते हैं, और उनकी आपसी निर्भरताएँ।

`crates/` के हर crate में एक छोटा README है, जिसमें उसका API और उसके डिज़ाइन निर्णय हैं। Guidedoc अपनी दस्तावेज़ तालिकाओं का स्वामी है और उन्हें रूपांतरणों के बीच दोबारा इस्तेमाल करता है, इसलिए स्थिर अवस्था में कोई रूपांतरण नई मेमोरी नहीं माँगता। यह सुझाव (hint) सहित संरचित निदान लौटाता है। होस्ट फ़ाइल, बजट, कैश, प्रकाशन और अंतर्निहित कंपाइलर सँभालता है। `--untrusted` क्षमताएँ सीमित करता है; यह प्रक्रिया का सैंडबॉक्स या सभी मूल संसाधनों की सीमा नहीं है। बीटा macOS और Linux पर भरोसेमंद दस्तावेज़ परियोजनाओं के लिए है।

## श्रेय और लाइसेंस

**Vikrant Rathore** ने इसे बनाया, **Ronak Rathore** और **Kanak Rathore** ने सहयोग दिया। इश्यू और पुल रिक्वेस्ट के ज़रिए योगदान का स्वागत है। दस्तावेज़ में नाम या हैंडल दिए जाते हैं, निजी ईमेल पते नहीं। Git की पहचान संबंधी सेटिंग अलग सँभाली जाती हैं।

Sphinx और Georg Brandl, Docutils और David Goodger, Typst, Tree-sitter और Max Brunsfeld, Graphviz, Rust परियोजना और उसके समुदाय, तथा Ginger Bill और Odin समुदाय का धन्यवाद।

Guidedog [GNU Lesser General Public License v3.0 या उसके बाद के संस्करण](LICENSE) (LGPL-3.0-or-later) के अंतर्गत जारी किया गया है, जो [GNU General Public License v3.0](COPYING) में अतिरिक्त अनुमतियाँ जोड़ता है। अन्य लाइसेंस शर्तों के लिए लेखकों से संपर्क करें। संपर्क का तरीका [NOTICE](NOTICE) में दिया गया है, साथ ही उन तृतीय-पक्ष घटकों की सूची भी, जिनके अपने लाइसेंस रहते हैं, जैसे Rust crates और शामिल फ़ॉन्ट। एक अतिरिक्त अनुमति Guidedog को Graphviz के साथ लिंक करने और वितरित करने देती है; देखें [NOTICE](NOTICE)।
