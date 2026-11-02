/* Guidedog web typography (_static/typography.js).
 *
 * Paragraph-wide Knuth-Plass line breaking and Liang pattern hyphenation for HTML
 * editions, copied into new projects so every site asset stays local and editable.
 *
 * Bundled third-party components:
 *
 * 1. Typeset (LinkedList and Knuth-Plass linebreak)
 *    Copyright (c) 2009-2010, 2026 Bram Stein. All rights reserved.
 *    BSD 2-Clause License:
 *    Redistribution and use in source and binary forms, with or without modification,
 *    are permitted provided that the following conditions are met:
 *    - Redistributions of source code must retain the above copyright notice, this list
 *      of conditions and the following disclaimer.
 *    - Redistributions in binary form must reproduce the above copyright notice, this
 *      list of conditions and the following disclaimer in the documentation and/or other
 *      materials provided with the distribution.
 *    THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
 *    EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 *    OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT
 *    SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
 *    INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
 *    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
 *    BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 *    CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN
 *    ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH
 *    DAMAGE.
 *
 * 2. Hypher (Liang pattern-trie hyphenation)
 *    Copyright (c) 2011, Bram Stein. All rights reserved.
 *    BSD 3-Clause License:
 *    Redistribution and use in source and binary forms, with or without modification,
 *    are permitted provided that the following conditions are met:
 *    1. Redistributions of source code must retain the above copyright notice, this list
 *       of conditions and the following disclaimer.
 *    2. Redistributions in binary form must reproduce the above copyright notice, this
 *       list of conditions and the following disclaimer in the documentation and/or other
 *       materials provided with the distribution.
 *    3. The name of the author may not be used to endorse or promote products derived
 *       from this software without specific prior written permission.
 *    THIS SOFTWARE IS PROVIDED BY THE AUTHOR "AS IS" AND ANY EXPRESS OR IMPLIED
 *    WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY
 *    AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
 *    OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
 *    EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 *    SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 *    INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 *    STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT
 *    OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 * 3. English hyphenation patterns (Hypher.en)
 *    Derived from TeX hyph-en-us.tex by Gerard D. C. Kuiken (version 2005-05-30),
 *    extending Donald E. Knuth and Frank M. Liang's plain TeX hyphen.tex patterns;
 *    unlimited copying and redistribution of the pattern values are permitted.
 */

(() => {
  const root = typeof globalThis !== "undefined" ? globalThis : window;
  const Typeset = root.Typeset || {};

  class LinkedListNode {
    constructor(data) {
      this.prev = null;
      this.next = null;
      this.data = data;
    }
    toString() { return this.data.toString(); }
  }

  class LinkedList {
    constructor() {
      this.head = null;
      this.tail = null;
      this.listSize = 0;
    }
    *[Symbol.iterator]() {
      for (let node = this.head; node !== null; node = node.next) yield node;
    }
    isLinked(node) {
      if (!node || this.isEmpty()) return false;
      return !(
        node.prev === null &&
        node.next === null &&
        this.tail !== node &&
        this.head !== node
      );
    }
    size() { return this.listSize; }
    isEmpty() { return this.listSize === 0; }
    first() { return this.head; }
    last() { return this.tail; }
    toString() { return this.toArray().toString(); }
    toArray() { return [...this]; }
    forEach(fn) { for (const node of this) fn(node); }
    contains(n) {
      if (!this.isLinked(n)) return false;
      for (const node of this) if (node === n) return true;
      return false;
    }
    at(i) {
      if (i >= this.listSize || i < 0) return null;
      let index = 0;
      for (const node of this) {
        if (i === index) return node;
        index += 1;
      }
      return null;
    }
    insertAfter(node, newNode) {
      if (!this.isLinked(node)) return this;
      newNode.prev = node;
      newNode.next = node.next;
      if (node.next === null) this.tail = newNode;
      else node.next.prev = newNode;
      node.next = newNode;
      this.listSize += 1;
      return this;
    }
    insertBefore(node, newNode) {
      if (!this.isLinked(node)) return this;
      newNode.prev = node.prev;
      newNode.next = node;
      if (node.prev === null) this.head = newNode;
      else node.prev.next = newNode;
      node.prev = newNode;
      this.listSize += 1;
      return this;
    }
    push(node) {
      return this.head === null ? this.unshift(node) : this.insertAfter(this.tail, node);
    }
    unshift(node) {
      if (this.head === null) {
        this.head = node;
        this.tail = node;
        node.prev = null;
        node.next = null;
        this.listSize += 1;
      } else {
        this.insertBefore(this.head, node);
      }
      return this;
    }
    remove(node) {
      if (!this.isLinked(node)) return this;
      if (node.prev === null) this.head = node.next;
      else node.prev.next = node.next;
      if (node.next === null) this.tail = node.prev;
      else node.next.prev = node.prev;
      this.listSize -= 1;
      return this;
    }
    pop() {
      if (this.isEmpty()) return null;
      const node = this.tail;
      if (node.prev === null) {
        this.head = null;
        this.tail = null;
      } else {
        node.prev.next = null;
        this.tail = node.prev;
      }
      this.listSize -= 1;
      node.prev = null;
      node.next = null;
      return node;
    }
    shift() {
      if (this.isEmpty()) return null;
      const node = this.head;
      if (node.next === null) {
        this.head = null;
        this.tail = null;
      } else {
        node.next.prev = null;
        this.head = node.next;
      }
      this.listSize -= 1;
      node.prev = null;
      node.next = null;
      return node;
    }
  }

  Typeset.LinkedList = LinkedList;
  Typeset.LinkedList.Node = LinkedListNode;

  // Knuth and Plass line breaking algorithm in JavaScript (BSD 2-Clause, Bram Stein).
  Typeset.linebreak = (function () {
    const linebreak = function (nodes, lines, settings) {
      const options = {
        demerits: {
          line: settings?.demerits?.line ?? 10,
          flagged: settings?.demerits?.flagged ?? 100,
          fitness: settings?.demerits?.fitness ?? 3000
        },
        tolerance: settings?.tolerance ?? 2
      };
      const lineLengths = lines;
      const startTolerance = settings?.tolerance ?? 2;
      const maxTolerance = settings?.maxTolerance ?? startTolerance + 4;
      let activeNodes, sum, breaks, tmp;

      function breakpoint(position, demerits, ratio, line, fitnessClass, totals, previous) {
        return {
          position,
          demerits,
          ratio,
          line,
          fitnessClass,
          totals: totals || { width: 0, stretch: 0, shrink: 0 },
          previous
        };
      }

      function isNonDiscardable(index) {
        const prev = nodes[index - 1];
        return (
          prev.type === "box" ||
          (prev.type === "penalty" && prev.penalty !== linebreak.infinity)
        );
      }

      function isFeasibleRatio(ratio, finalLine) {
        if (ratio < -1 || ratio === linebreak.infinity || !isFinite(ratio)) return false;
        return finalLine ? true : ratio <= options.tolerance;
      }

      function computeCost(start, end, active, currentLine) {
        let width = sum.width - active.totals.width;
        const lineLength = currentLine < lineLengths.length
          ? lineLengths[currentLine - 1]
          : lineLengths[lineLengths.length - 1];
        if (nodes[end].type === "penalty") width += nodes[end].width;
        if (width < lineLength) {
          const stretch = sum.stretch - active.totals.stretch;
          return stretch > 0 ? (lineLength - width) / stretch : linebreak.infinity;
        }
        if (width > lineLength) {
          const shrink = sum.shrink - active.totals.shrink;
          return shrink > 0 ? (lineLength - width) / shrink : -linebreak.infinity;
        }
        return 0;
      }

      function computeSum(breakPointIndex) {
        const result = { width: sum.width, stretch: sum.stretch, shrink: sum.shrink };
        const node = nodes[breakPointIndex];
        if (node.type === "glue") {
          result.width += node.width;
          result.stretch += node.stretch;
          result.shrink += node.shrink;
        }
        return result;
      }

      function mainLoop(node, index) {
        let active = activeNodes.first();
        let next = null;
        let ratio = 0;
        let demerits = 0;
        let currentLine = 0;
        let currentClass = 0;
        const finalLine = node.type === "penalty" && node.penalty === -linebreak.infinity;

        // Keep active nodes sorted by line number across candidate insertions.
        while (active !== null) {
          const candidates = [
            { demerits: Infinity },
            { demerits: Infinity },
            { demerits: Infinity },
            { demerits: Infinity }
          ];
          while (active !== null) {
            next = active.next;
            currentLine = active.data.line + 1;
            ratio = computeCost(active.data.position, index, active.data, currentLine);
            if (ratio < -1) activeNodes.remove(active);

            if (isFeasibleRatio(ratio, finalLine)) {
              const badness = 100 * Math.pow(Math.abs(ratio), 3);
              const linePenalty = Math.pow(options.demerits.line + badness, 2);
              if (node.type === "penalty" && node.penalty >= 0) {
                demerits = linePenalty + Math.pow(node.penalty, 2);
              } else if (node.type === "penalty" && node.penalty !== -linebreak.infinity) {
                demerits = linePenalty - Math.pow(node.penalty, 2);
              } else {
                demerits = linePenalty;
              }
              const activeNode = nodes[active.data.position];
              if (node.type === "penalty" && activeNode.type === "penalty") {
                demerits += options.demerits.flagged * node.flagged * activeNode.flagged;
              }
              if (ratio < -0.5) currentClass = 0;
              else if (ratio <= 0.5) currentClass = 1;
              else if (ratio <= 1) currentClass = 2;
              else currentClass = 3;

              if (Math.abs(currentClass - active.data.fitnessClass) > 1) {
                demerits += options.demerits.fitness;
              }
              demerits += active.data.demerits;
              if (demerits < candidates[currentClass].demerits) {
                candidates[currentClass] = { active, demerits, ratio };
              }
            }

            active = next;
            if (active !== null && active.data.line >= currentLine) break;
          }

          const tmpSum = computeSum(index);
          for (let fitnessClass = 0; fitnessClass < candidates.length; fitnessClass += 1) {
            const candidate = candidates[fitnessClass];
            if (candidate.demerits < Infinity) {
              const newNode = new Typeset.LinkedList.Node(breakpoint(
                index,
                candidate.demerits,
                candidate.ratio,
                candidate.active.data.line + 1,
                fitnessClass,
                tmpSum,
                candidate.active
              ));
              if (active !== null) activeNodes.insertBefore(active, newNode);
              else activeNodes.push(newNode);
            }
          }
        }
      }

      for (let tryTolerance = startTolerance; tryTolerance <= maxTolerance; tryTolerance += 1) {
        options.tolerance = tryTolerance;
        activeNodes = new Typeset.LinkedList();
        sum = { width: 0, stretch: 0, shrink: 0 };
        breaks = [];
        tmp = { data: { demerits: Infinity } };
        activeNodes.push(
          new Typeset.LinkedList.Node(breakpoint(0, 0, 0, 0, 0, undefined, null))
        );

        for (let index = 0; index < nodes.length; index += 1) {
          const node = nodes[index];
          if (node.type === "box") {
            sum.width += node.width;
          } else if (node.type === "glue") {
            if (index > 0 && isNonDiscardable(index)) mainLoop(node, index);
            sum.width += node.width;
            sum.stretch += node.stretch;
            sum.shrink += node.shrink;
          } else if (node.type === "penalty" && node.penalty !== linebreak.infinity) {
            mainLoop(node, index);
          }
        }

        if (activeNodes.size() !== 0) {
          let maxPosition = 0;
          for (const activeNode of activeNodes) {
            if (activeNode.data.position > maxPosition) maxPosition = activeNode.data.position;
          }
          for (const activeNode of activeNodes) {
            if (
              activeNode.data.position === maxPosition &&
              activeNode.data.demerits < tmp.data.demerits
            ) {
              tmp = activeNode;
            }
          }
          while (tmp !== null) {
            breaks.push({ position: tmp.data.position, ratio: tmp.data.ratio });
            tmp = tmp.data.previous;
          }
          return breaks.reverse();
        }
      }

      return [];
    };

    linebreak.infinity = 10000;
    linebreak.glue = function (width, stretch, shrink) {
      return { type: "glue", width, stretch, shrink };
    };
    linebreak.box = function (width, value) {
      return { type: "box", width, value };
    };
    linebreak.penalty = function (width, penalty, flagged) {
      return { type: "penalty", width, penalty, flagged };
    };
    return linebreak;
  })();

  // Hypher Liang pattern-trie hyphenator (BSD 3-Clause, Bram Stein).
  function Hypher(language) {
    this.trie = this.createTrie(language.patterns);
    this.leftMin = language.leftmin;
    this.rightMin = language.rightmin;
    this.exceptions = {};
    if (language.exceptions) {
      language.exceptions.split(/,\s?/g).forEach(function (exception) {
        const marker = new RegExp(exception.indexOf("=") !== -1 ? "=" : "-", "g");
        this.exceptions[exception.replace(marker, "")] = exception.split(marker);
      }, this);
    }
  }

  Hypher.prototype.createTrie = function (patternObject) {
    const tree = { _points: [] };
    for (const size in patternObject) {
      if (Object.prototype.hasOwnProperty.call(patternObject, size)) {
        const patterns = patternObject[size].match(new RegExp(".{1," + (+size) + "}", "g"));
        patterns.forEach(function (pattern) {
          const chars = pattern.replace(/[0-9]/g, "").split("");
          const points = pattern.split(/\D/);
          let t = tree;
          chars.forEach(function (c) {
            const codePoint = c.charCodeAt(0);
            if (!t[codePoint]) t[codePoint] = {};
            t = t[codePoint];
          });
          t._points = points.map(function (p) { return p || 0; });
        });
      }
    }
    return tree;
  };

  Hypher.prototype.hyphenateText = function (str, minLength) {
    const limit = minLength || 4;
    const words = str.split(/\b/g);
    return words.map(function (word, i) {
      if (word.indexOf("/") !== -1) {
        if (i === 0 || i === words.length - 1 || /\s+\/|\/\s+/.test(word)) return word;
        return word + "\u200B";
      }
      if (word.length <= limit) return word;
      return this.hyphenate(word).join("\u00AD");
    }, this).join("");
  };

  Hypher.prototype.hyphenate = function (word) {
    if (Object.prototype.hasOwnProperty.call(this.exceptions, word)) {
      // A copy: callers may edit the result, and the stored exception must stay intact.
      return this.exceptions[word].slice();
    }
    if (word.indexOf("\u00AD") !== -1) return [word];

    const padded = "_" + word + "_";
    const characters = padded.toLowerCase().split("");
    const originalCharacters = padded.split("");
    const wordLength = characters.length;
    const points = [];
    const characterPoints = [];
    const result = [""];

    for (let i = 0; i < wordLength; i += 1) {
      points[i] = 0;
      characterPoints[i] = characters[i].charCodeAt(0);
    }
    for (let i = 0; i < wordLength; i += 1) {
      let node = this.trie;
      for (let j = i; j < wordLength; j += 1) {
        node = node[characterPoints[j]];
        if (!node) break;
        const nodePoints = node._points;
        if (nodePoints) {
          for (let k = 0; k < nodePoints.length; k += 1) {
            points[i + k] = Math.max(points[i + k], nodePoints[k]);
          }
        }
      }
    }
    for (let i = 1; i < wordLength - 1; i += 1) {
      if (i > this.leftMin && i < wordLength - this.rightMin && points[i] % 2) {
        result.push(originalCharacters[i]);
      } else {
        result[result.length - 1] += originalCharacters[i];
      }
    }
    return result;
  };

  // Derived from hyph-en-us.tex; its copyright and distribution notice are retained above.
  Hypher.en = {
    leftmin: 3,
    rightmin: 3,
    patterns: {
      "5": [
        "_ach4_af1t_al3t_an5c_ang4_ant4_ar5s_as3c_as1p_as1s_au1d_av4i_awn4_ba4g_ber4_bri2_ca4",
        "t_co3e_co4r_de3o_do4t_du4c_eer4_el5d_en3g_en3s_eye5_fes3_gi5a_gi4b_go4r_hes3_het3_hi",
        "3b_hov5_id4l_im3m_ine2_in2k_in3s_ir5r_is4i_ju3r_la4m_len4_lep5_lev1_li4g_li2n_li3o_l",
        "i4t_mis1_ni4c_odd5_or3c_or1d_or3t_oth3_out3_pi4e_pi2t_ra4c_ree2_res2_ri4g_ro4q_ru4d_",
        "se2n_til4_to4p_un1a_un1e_un5k_un5o_un3u_ure3_us5aa5bala5banabi5aab3ula4carac1er4a2ci",
        "a3cieac1ina3cioac3ulac4uma3diaa3dioa3dita5diuad4lead3owad4sua3ducad5uma4gabaga4nage4",
        "o4ageu4ag4l3agogag5ula3ic_ai5lya4i4nain5oak1enal5abal3ada4lar4aldiali4ea4ly_4alys5al",
        "yt3alyzam5abam3agam3icam5ifam1ina5mona3naran1dla5neea3nena3neuan1glan3ioa3nipan3ita3",
        "niuan5otan2saan4snan2span4st4antoan2tran4twan3uaan3ula5nurapar4ap5at4aphiap3inapoc5a",
        "que5ar3alara3par4ata5rauaraw4ar4dra3reear4fiar4flar4imar3ioar2izar2mia3rooarre4ar4sa",
        "ar2shas4abashi4a3siba3sicask3ia4socas5phas4shas1trat5acat5apate5cat5evat4ho4ati_a5ti",
        "aat1icat3ifa4toga2toma4topa4tosat4skat5teat4that5uaat5ueat3ulaugh3au3guau4l2aun5dau1",
        "thav3aga5vanav3igav5oca1vor3awayaw4lyax4icax4iday5alazz5iba4gebal1aban4eban3ib3berbe",
        "ak4beat34be2dbe3dabe3debe3dibe3gibe5gube1libe3lo4be5mbe5nu4bes4be3spbe5trbe3twbe5yob",
        "i5enbi4er2b3ifbin4dbi5oubi3trb5itzb4le_blen4b3lisbne5gbod3ibon4a5bor_bor5d5bore5bori",
        "5bos4b5otaboth5bo4to4brit2b5s2bsor4bu4gabu3libumi4bu3re5bust4butab5utoca1blcach44cag",
        "42c5ah4calocan5dcan4ecany44casyca4thccha5cci4accon44ced_5cel_3cell3cenc4ceni3cent4ce",
        "saces5t4ched5chio3chitchi2z3cho2ch4ticia5r4cierci4la3cilic4inac1ing5cinocion44cipeci",
        "3ph2c1it1c4l44clarcle4m4clicclim4co5agco4grcol5i5colocon4ac4onecon3gcon5tco3paco4pl4",
        "corbcos4ecove4cow5acoz5eco5zi5credcre4vcri5fc4rincris4cru4d4c3s2cta4bc3terctu4r5culi",
        "cu2mac3umecu4micu3picu5py3c4ut4cutrdach4da2m2dan3gdard5dark54dary4dato5dav4dav5edeaf",
        "52d1ed4dee_de5ifde5lo5dem_de3node3nude3padepi4de2pud4erh5dermder5sd2es_de1scde4sude2",
        "todia5bd4ice3dictdi3ge1dina5dinidio5gdi4pldi1re5disid2iti1di1v4d5la3dle_3dled2d3lo4d",
        "5lu4d1n4do5de2d5ofdo4ladoli4doni4doo3ddop4p4drai5drendri4bdril4dro4p4drow2d1s2d1u1ad",
        "1ucadu5eld3uledu4pedy4sedys5pe1a4be3actea4gee5andear3aear4cear5kear2tea5spe3asseast3",
        "eav5ieav5oe4bene4bite4cadecca5e4cibec3imeci4te2cole2corec4tee4cul2e2da4ed3dede4se3di",
        "aed3ibed3imed1itedi5ze4doledon2e4drie4duleed3ieel3iee4lyee4naee4p1ee2s4eest4ee4tye4f",
        "icefil43efit4egaleger4eg5ibeg4ice4go_e4goseg1ule5gureher4ei5gle3imbe3infe1ingeir4dei",
        "t3eei3the5itye4judeki4nek4lae4la_e4lace4lawe3lea5elece4lede5lene1lese5lime3lioe2lis4",
        "ellaello4e5locel5ogel2shel4tae5ludel5uge4mace4mage5mane2mele4metemi4ee4misem3izemo4g",
        "em3pie4mulemu3ne5neae5neeen3eme3newe5niee5nile3nioen3ite5niu5enizeno4ge4nosen3oven4s",
        "wen3uaen5ufe3ny_4en3ze4oi4eo3reeo4toe5oute3paie5pelephe4e4plie3proep4she4putera4ber3",
        "arer4bler3ch2ere_ere4qeret4e1rio4eriter4iueri4ver3m4er3noer5obe5rocero4rer1ou4ertler",
        "3tweru4te1s4ae2scae3scres5cue1s2ee2sece3shae2sice2sidesi4ues4mie2sole2son2estre2sure",
        "ta4be3teoet1icetin4e5tire3trae3treet3uaet5ymeu3roeute4eu5tre2vase5veaev1erev3idevi4l",
        "e4vinevi4ve5voce4wage5weeewil5e3wit5eye_fa3blfab3rfa4cefain4fa3ta4fatofeas44feca5fec",
        "tfe3life4mofen2d5ferrf4fesf4fief4flyfic4i4ficsfi3cufil5i4fily5finafi2nefin4nflin4f2l",
        "y5fon4tfor4ifra4tf5reafril4frol5fu5elfu5nefu3rifusi4fus4s4futa5gal_3galiga3log5amo4g",
        "anogass4gath3geez44gely4geno4genyge3omg4ery5gesigeth54getoge4ty4g1g2g3gergglu5gh3ing",
        "h4to1gi4agia5rg4icogien5gir4lg3isl5glasgli4bg3ligglo3rg4na_g2ning4niog4nongo3isgo3ni",
        "5gos_g4raigran24graygre4n4gritgruf4g5ste4gu4tgy5rahach4hae4mhae4th5aguha3lahan4ghan4",
        "khap3lhap5thar2dhas5shaun4haz3a1head3hearh5elohem4phena4heo5rh4erah3ernh3eryhi5anhi4",
        "cohigh5h4il2h4inahir4lhi3rohir4phir4rhis4s4h1l4hlan4hmet4h5odshoge4ho4mahome3hon4aho",
        "5ny3hoodhoon4ho5ruhos4ehos1p1houshree54h1s2h4tarht1enht5eshun4thy3pehy3ph4iancian3ii",
        "a5peiass4i4atuib5iaib3inib3lii5bun4icam5icap4icaricas5i4cayiccu44iceoi5cidi2cipi4cly",
        "4i1cr5icrai4cryic4teictu2ic4umic5uoi3curi4daiide4si5dieid3ioid1itid5iui3dlei4domid3o",
        "wid5uoied4eield3ien4ei5enni1er_i3esci1estif4fri3fieiga5bi3gibig3ilig3inig3iti4g4lig3",
        "orig5oti5greigu5iig1ur4i5i4i3legil1erilev4il3iail2ibil3io2ilitil2izil3oqil4tyil5uri4",
        "mag4imet4imitim4nii3mon4inavi3nee4inga4inge4ingi4ingo4ingui5ni_i4niain3ioin1is2i1noi",
        "no4si4notin3se2int_i5nusioge4io2grion3iio5phior3iio5thi5otiio4toi4ourip4icip3uli3qua",
        "ira4bi4racird5ei4refi4resir5giir4isiro4gir5ulis5agis3arisas52is1cis3chis3eris3ibisi4",
        "di5sis4is4k4ismsis2piis4py4is1sis1teis1tiis5us4ita_i4tagi3tani3tatit4es4itiait3igi2t",
        "im2itio4itis4itonit5ryi5tudit3ul4itz_iv5ioiv1it4izarjac4qjer5s5judgkais4ke5like4ty5k",
        "2ick4illkilo5k4in_kin4gk5ish4kleyk5nes1k2nokosh4kro5n4k1s2l4abolaci4l4adela3dylag4nl",
        "am3o3landlar4glar3ilas4elbin44l1c2ld5isl4drile4bileft55leg_5legg4len_3lenc1lentle3ph",
        "le4prler4e3lergl4ero5lesq3lessl3eva4leye4l1g4lgar3l4gesli4agli2amli4asli5bi4licsl4ic",
        "ul3icyl3ida3lidil4iffli4fl3lighlim3ili4mol4inalin3ili5og4l4iqlis4pl2it_l3kallka4tl4l",
        "awl5leal3lecl3legl3lell5lowl5metl4modlmon42l1n2lo4cil5ogo3logu5longlon4ilood5lop3il3",
        "opmlora45los_los4tlo4ta2loutlpa5bl3phal5phil3pit2l1s2l4sielt5aglten4lth3iltis4lu3brl",
        "uch4lu3cilu3enlu5idlu4ma5lumiluo3rluss4l5venly5mely3no2lys4l5ysema2cama4cl5magnmaid5",
        "4maldmar3vmas4emas1t5matemath3m5bilmbi4v4med_mel4tmen4a4menemen4imens43mentme5onme4t",
        "ame1tem4etrmid4amid4gm4illmin4a3mindmin4tm4inumiot4mis5l4mithm4nin4mocrmo2d1mo4gomoi",
        "s2mo3memo3spmoth3m5ouf3mousm3petmpi4am5pirmp5ismpov5mp4tr4m1s25multn4abu4nac_na4can5",
        "actna4li4naltnank4nar3c4narenar3inar4ln5armnas4c3nautnav4e4n1b4ncar5n3chanc1innc4itn",
        "4dain5danndi4bn1ditn3dizn5ducndu4rnd2wen3earneb3u5neck5negene4lane5mine4mo4nenene4po",
        "n2erener4r2nes_4nesp2nest4neswn5even4gabn3gelng5han3gibng1inn5gitn4glangov4ng5shn4gu",
        "m4n1h4nhab33n4iani3anni4apni3bani4blni5dini4erni2fin5igrnin4g5nis_n4ithni3trn3ketnk3",
        "innmet44n1n2nni4vnob4ln5oclnoge4no4mono3mynon5i4noscnos4enos5tno5ta3nounnowl32n1s2ns",
        "5abnsid1nsig4n4socns4pen5spinta4bn5tibnti2fnti4pnu5enn3uinnu1men5umi3nu4nnu3troard3o",
        "as4eoat5io5barobe4lo2binob3ulocif3o4cilo4codocre3od3icodi3oo2do4odor3o5engoe4tao5geo",
        "o4gero3gieog3ito4groogu5i2ogynohab5oiff4o3ingo5ismo3kenok5ieo4lanold1eol3ero3letol4f",
        "io3liao5lilo5lioo5livolo4rol5plol3ubol3uno5lusom5ahoma5lom2beom4blo4meto3miao5midom1",
        "ino4monom3pion4aco3nanon5doo3nenon4guon1ico3nioon1iso5niuonsu4on5umonva5ood5eood5ioo",
        "p3io3ordoost5ope5dop1ero3pito5pono5ra_ore5aor3eiorew4or4guo5rilor1ino1rioo3riuor2mio",
        "rn2eo5rofor5pe3orrhor4seorst4or4tyo5rumos3alos4ceo5scrosi4uos4paos4poos2tao4tano4tes",
        "o3tifo3tisoto5sou3blou5etoun2dov4eno3visow3elown5ipa4capa4cepac4tpain4pan4apa3nypa4p",
        "u3parepa2te3pe4a2p2ed3pede3pedipee4dpe4lap4encpe5onp4erip4ernper3ope5ruper1vph4erph1",
        "ic5phie3phiz3phobpho4rpian4pi4cyp5idapi3de5pidi3piecpi3enpi3lop4in_pind4p4ino3pi1opi",
        "on4p3ithpi2tu2p3k21p2l23planpli3a4pligpli4nploi4plu4m5pod_po5em5po4gpoin2po4ni1p4orp",
        "o4rypos1spo4ta5pounp4pedp5pelp3penp3perp3petpre3rpre3vpri4spro3lpro1t2p1s2p4sibpti3m",
        "ptu4rpul3cpur4r5putepu3trqua5v2que_3quer3quetra3bir5aclraf4tra4lor2amir4anira5norar5",
        "crare4rau4tr4babr4bagrbi4fr2binrcen4r3charc4itrcum3r4dalrdi4ardin4re1alre3an5reavre4",
        "aw2r2edre1dere2fere3fire4fyre5itre1lire5lure1pur1er4r4erirero4re5rur4es_res2tre4whrg",
        "3err3getr3gicrgi4nr5gisr5gitrgo4n4rhalria4bri4agrib3ar4ice4ricir4icori1erri5et5rigir",
        "im5i3rimor2inarin4drin4erin4g5riphri2plr4is_ris4cr3ishris4pri2turiv3ir3ketrk4ler2led",
        "r4ligr4lisr3lo4rma5cr3menr4mior3mitr4narr3nelr4nerr5netr3neyr5nicr3nitr3nivr4nourob3",
        "lro3crro1ferom4irom4pron4e1room5rootror3iro5roros4sro4tyro4var4pear3petrp4h4rre4crre",
        "4fr4reorri4orri4vrron4rros4rrys4r3secrs3esr5sharson3r4tagr3tebrte5ort5ibrti4dr3tigr4",
        "tivr3trirt4shru3enru4glru3inrunk5r5uscr3venr3veyr3vicrvi4v2s1ab5sacks3actsal4msa5los",
        "al4t3sancsa5tasat3usca4pscav5s4ced4sceis4cess4choscle5s4cliscof4seas4sea5w3sect4s4ed",
        "s5edlseg3rse1le5self5selv4seme4sencsen4dsen5gs4erlser4os1e4sse5shses5tsew4ish1er5she",
        "vsh1insh3io3shipshiv5shon3shor4s5icc5sidisil4e4sily2s1ins2inas3ing5sionsir5as3kets3l",
        "atsman3smel4s5menso4cesoft35solvsona4son4gsor5csor5dso5vi5spaispa4n2spers2phespho5sp",
        "il44spios4plys4ponspor44spotssas3s2s5cs3sels5sets4siess4lis2tags2tals4tedste2ws3thes",
        "4ti_s5tias1tics4ties3tif5stirs1tles4top4stry4st3wsu1alsu4b3su2g3su5issuit3sum3isyn5o",
        "4tacita5do4taf4ta5latal3i4talkta5mota5pltar4a4tarc4taretas4eta5sytaun44teattece45tec",
        "t2t1edte5dite5gi3tel_teli45tels3tenc3tend1tentte5peter3c1teriter5v4tes_4tessthan44th",
        "ea3thet4thil4thooti4ab2ti2b4tickt4ico5tidi3tienti5fy5tigu4timp2t1int2ina3tiniti5octi",
        "3sa3tisetis4mti5sotis4pti3tltiv4ati3zatlan43tle_3tled2t1n24todoto2grto5icto2matom4bt",
        "o3my4tono4tonyto2ra5tour4touttra3btras4tre5ftre4m5tria2trimtri4vtro3vtru5itrus44t1s2",
        "4t3t2t4testu3artu4bi4tuf45tu3i3ture5turitur5otu5ry4t1watwis4type3ty5phua5nauan4iuar2",
        "duar3iuar3tu4belu3beru1b4iuci4buc4itucle3ud3erudev4u1dicud5isu5ditu4donud4siu4eneuen",
        "s4ug5inu1inguir4muita4ula5bulch4u1lenul4giu5lia4ul3mu1l4oul1ti4ultuul5ulum5abum4biun",
        "at4un4erun4imu2ninuni3vun3s4un4swu4orsu5piauptu44ura_u4ragu4rasur4beur4fru3rifur1inu",
        "3riou1ritur3izur4nouros4ur4peur4piurti4u5sadu5sanus4apus3ciuse5au5siau3sicus5slus1tr",
        "usur4uta4bu3tat4ute_4utel4utenu4tisu4t1lut5ofuto5gu5tonu4touvac3uva4geval5oval1uva5m",
        "ova5piv3el_ve4lov4elyv4erdv4e2s4ves_ve4teve4ty5vian5vidivi5gnv2incvin5d4vingvio3lvi1",
        "ouvi5rovi3sovi3su4vitivit3r4vityvo4lav5ole5volt3volvvom5ivori4vo4ryvo4taw5ablwag5owa",
        "it5w5al_war4twas4twa1tewed4nweet3wee5vwel4lwest3win4g3wisewith3wl4eswl3inws4pew5s4tx",
        "ac5ex4agoxer4ixe5roxhil5xi5dix4imexpe3d3yar4yc5erych4eycom4ycot4y4erfympa3yn5icy4o5g",
        "y4onsy4pedyper5y4pocyp2tayra5myr5iay3s2eys3io3ysisys3taysur4yt3icz5a2bze3rozo5ol4z1z",
        "2_to2qans3vbbi4tbio5m4chs_cin2qd3tabedg1le1loa3fichk3shall1fll3tealue1pm2an_m2en_ng1",
        "hoo3noura1orrk1ho6rks_rz1sc1synctch1cuea1mz2z3w"
      ].join(""),
      "7": [
        "_ad4der_anti5s_ar4tie_aster5_be5sto_but4ti_cam4pe_capa5b_car5ol_de4moi_earth5_gen3t4",
        "_hand5i_hero5i_hon5ey_im5pin_lat5er_mag5a5_mar5ti_me5ter_mist5i_muta5b_or5ato_ped5al",
        "_pe5tit_re5mit_se5rie_sta5bl_ten5an_tim5o5_under5_ven4dea4lenti5a5lysta4matisa4m5ato",
        "an5est_a4pillaar5adisa5ratioar5ativar4chanar5dinear5inat5a5si4ta5ternaat5omizbad5ger",
        "ban5dagbina5r43bi3tio3bit5uabuf4fercall5incast5ercas5tigccompa55chanic5chine_5cific_",
        "5cratic4c3retacul4tiscur5a4b4c5utivdel5i5qdem5ic_de4monsdenti5fdern5izdi4latodrag5on",
        "5drupliec5essaec5ifiee4compee4f3ereefor5ese4fuse_el5ativel5ebrae4l5ic_el5igibe4l3ing",
        "em5igraem3i3niemoni5oench4erent5age4enthesep5recaep5ti5b4erati_er5encee4sage_e4sages",
        "e4sert_e4sertse4servaes5idenes5ignaesis4tees5piraes4si4bestruc5e5titioet5itiv4f3ical",
        "4ficatefill5ingani5za4g3o3na5graph_4graphy4gress_hang5erh5a5nizharp5enhar5terhel4lis",
        "hith5erhro5niziam5eteia4tricic4t3uainer4ari5nite_5initioinsur5aion4eryiphras4iq3ui3t",
        "5i5r2izis5itiviso5mer4istral5i5ticki2t5o5mi4v3er_i4vers_iv3o3ro4jestiek5iness4lateli",
        "lev4er_lev4eralev4ersliar5iz5ligatelink5er5liticalloqui5l3o3nizlo4ratol5ties_5lumnia",
        "4matizam4b3ing5metricme5triem5i5liemin5glim5inglymis4er_m5istrymo5lestmon4ismmon4ist",
        "mpa5rabmula5r4nag5er_ncour5and5est_nge4n4en5o5mizno4rarynov3el3nsta5bln4t3ingo5a5les",
        "octor5aod5uct_od5uctso2g5a5rog5ativoint5eroist5eno5litiool5ogizom5atizom5erseom5etry",
        "5ommend4operagor5alizor5angeor5est_4oscopios5itivo5statiotele4goth5esiounc5erover4ne",
        "para5blpar5age5pathicpa4tricpera5blperi5stper4mal5phistipi4grappref5acpre5tenprin4t3",
        "prof5itput4tedput4tinration4rb5ing_r5ebratrec5ollre5fer_r4en4tare4posiress5ibre5stal",
        "re4ti4zre5versre5vertrev5olurip5licri3ta3br5ited_rit5er_rit5ersr4ming_rom5etero5n4is",
        "ros5perrtroph45sa3tioscan4t55scin4dscour5asmol5d45sophics5ophizsqual4lsspend4stern5i",
        "5stratuta5blestal4listen4tagter5iesteri5za5ternit5thodicthor5ittho5riztill5intion5ee",
        "to5cratton4alitrac4ittrac4tetra5ventri5ces5triciatro5pheuar5antu4b5inguiv4er_ul4li4b",
        "u4m3ingun4ter_upport5uri4ficus5tereuti5lizution5avar5iedver5encvermi4n4v3idenv3i3liz",
        "wea5riewill5inxe4cutoxpecto5ylla5bl_con5gr_dri5v4_eu4ler_ge4ome_ge5ot1_he3mo1_he3p6a",
        "_he3roe_in5u2t_ne6o3f_noe1th_poly1s_pre1am_sem4ic_semid6_semip4_semir4_semiv4_spin1o",
        "_to6pog_we2b1l_re1e4ca5bolicaf6fishanal6ysano5a2canti1reat6tes_augh4tlau5li5fbi5d2if",
        "bil2labblan2d1blin2d1blon2d2bor1no5but2ed_but4tedcad5e1mchie5vocig3a3rcous2ticri3tie",
        "de4als_dic1aiddif5fra2d1lead2d1li2eeo3grapfeb1ruafermi1ogen2cy_get2ic_4g1lishg2n1or_",
        "h2t1eouifac1etith5i2lje1re1m1kovianlea4s1al2l3ishlo2ges_ly5styrma1la1p3militamol1e5c",
        "mono3chmono1s6ne3back2ne1skin1diecknge5nesnom1a6lno1vembns5ceivobli2g1odel3liodit1ic",
        "onom1ico3normaop1ism_orth1ripal6matpara3mepli5narpoin3capre1neupro2g1e3pseu2dp5trol3",
        "quasir6quasis6r3a3digriv1o1lr5le5quro1bot1ro3meshro1tronsales3csales5ws4chitzscy4th1",
        "shoe1stsid2ed_side5stside5swso2lute3s2pacespe3ciospher1o1s2tampstyl1ista3gon_talk1a5",
        "t1a1min5tar2rhter2ic_tho3dont1li2ertolo2gytr4ial_tro3fittsch3ieturn3arty2p5alu5do3ny",
        "u2r1al_uri4al_va6guervaude3vvoice1pwave1g4wide5spwo4k1eny5che3dz3o1phr"
      ].join(""),
      "6": [
        "_am5at_ani5m_an3te_ar4ty_atom5_ba5na_bas4e_be5ra_be3sm_can5c_ce4la_cit5r_de3ra_de3ri",
        "_des4c_dumb5_eas3i_el3em_enam3_er4ri_ge5og_han5k_hi3er_hon3o_idol3_in3ci_la4cy_lath5",
        "_leg5e_lig5a_mal5o_man5a_mer3c_mon3e_mo3ro_mu5ta_of5te_os4tl_pe5te_pio5n_pre3m_ran4t",
        "_rit5u_ros5t_row5d_sci3e_self5_sell5_sing4_ting4_tin5k_ton4a_top5i_tou5s_un3ce_ve5ra",
        "_wil5iab5erdab5latab5rogac5ardac5aroa5ceoua5chetac5robact5ifad4dinad5er_ad3icaadi4er",
        "ad5ranaeri4eag5ellag3onia5guerain5inait5enal3enda5le5oal4ia_al5lev4allica5log_ama5ra",
        "am5ascam5eraam5ilyami4noamor5iamp5enan3age3analyan3arcanar4ia3natiande4san3disan4dow",
        "ang5iea4n1ica3niesan3i3fan4imea5nimia5ninean3ishan4kli5annizanoth5an4scoans3poan4sur",
        "antal4an4tieap5eroa3pherap3itaa3pituap5olaapor5iapos3taps5esar3acta5radearan4gar5av4",
        "arbal4ar5easar3enta5ressar5ialar3iana3rietar5o5da5ronias3anta5sia_as3tenasur5aat3abl",
        "at3aloat5echat3egoat3en_at3eraater5nat3estath5ema5thenath5omat5i5bat3ituat5ropat4tag",
        "at3uraau5sibaut5enave4noav3eraav5ernav5eryavi4erazi4erbarbi5bari4abas4sibbi4nabe5nig",
        "be5strbet5izbi3lizbi5netbi3ogrblath55blespblun4tbol3icbom4bibon5at4b1orabound3broth3",
        "bunt4ibus5iebuss4e3butiocab3inca5denca3latcal4lacan4iccan5iscan3izcan4tyca5percar5om",
        "4cativcav5alccou3t4ceden2cen4ece5ram3cessic5e4ta4ch3abcheap3che5lo3chemich5enech3er_",
        "ch3ers4ch1in5chinici2a5b3cinatcin3emc5ing_4cipic4cista4cisticit3iz5clareco3inccol3or",
        "com5ercop3iccoro3ncras5t5crat_cre3at5criticro4plcrop5ocros4ect5angc5tantc4ticuctim3i",
        "cu5ity3cultucu5riacuss4icu4tie2d3a4b4dativdeb5itde4bondecan4de4cilde5comdeli4e3demic",
        "de5mildemor5de4narde2s5odes3tide3strdev3ild3ge4t1d4i3adi4cam5di3en3dine_di5nizdirt5i",
        "d4is3t3dles_4dlessdo5lordom5izdo3nat4d5outdrea5rduc5er4duct_4ductsdum4beead5ieea5ger",
        "eal5ereal3oueam3erear5esear4icear4ileart3eeat5eneath3ie5atife4a3tueav3ene4bel_e4bels",
        "ecan5cec5ifye5citee4clame4cluse4comme4concec3oraeco5roe4cremec4tanec3ulae4d1ered3ica",
        "ed5ulo5eficie3fineeg5inge5git5e5instej5udielan4delaxa4el3egae4l1ere3libeel3icae3lier",
        "el5ishe3liv3el4label3op_em5anaem3icaem1in2em5ineem5ishe5miss5emnizem5ulaen5amoe4nant",
        "en3dicen5eroen5esien5esten3etren5icsen3isheop3areo5rolep5ance3pente4prece4predep3reh",
        "e4probep5utaequi3l4erander4chee3realere5coere3iner5el_er3emoer5ena4ereneer3enter5ess",
        "er3este1ria45ericke3rieneri4erer3inee4rivaer4nis4ernit5ernizer3setert3er5erwaues5can",
        "es5ecres5encesh5ene2s5imes4i4ne5skines3olues5onaes3peres4preestan4es3tiges5tim4es2to",
        "e3stone5stroes5urreten4dethod3e5tideeti4noet5onaet3ricet5rifet3roget5roseuti5leva2p5",
        "ev5astev3ellevel3oe5vengeven4ie5verbew3ingfall5e4fa4mafam5isfar5thfa3thefault5feath3",
        "fend5ef5fin_f2f5is2f3ic_f3icanf3icenfi3cer5ficia5ficiefi5delfight5fin2d5f1in3gfis4ti",
        "f5lessflo3refon4defo5ratfor5ayfore5tfort5afres5cfu4minga5metgan5isga3nizgar5n44gativ",
        "gel4inge5lisge5lizge4natge5nizgh5out5gicia5gies_g3imen3g4in_gin5ge5g4insglad5ignet4t",
        "3g4o4ggondo5go5rizgor5ou4grada3guard5gui5t2g5y3nh3ab4lhala3mhan4cihan4cy5hand_hang5o",
        "han4teha3ranha5rashard3ehar4lehe4canh5ecathe5do5he3l4ihel4lyhen5athera3pher4bahere5a",
        "h5erouhe2s5phet4edhimer4hion4ehis3elhlo3rih5odizhol5ar3hol4ehor5atho5rishort3eho5sen",
        "house3hov5elhro3pohu4minhun5kehus3t4h4wart4ian4ti4ativib3eraib5ertib5it_ib5itei2b5ri",
        "i4car_i4caraic5inaic3ipai2c5ocic3ulaid5ancide3alid5ianidi4aridi5ou5ie5gaien5a4i3enti",
        "if5eroiff5en4ific_ig3eraight3iil3a4bi4ladei2l5amila5rail4istill5abim3ageima5ryim5ida",
        "imi5lei5miniim3ulai4n3auincel4in3ceri5nessin5genin3ityi4no4c2in4thion3atip4re4iq5uef",
        "iq3uidire4dei4rel4iri5deiri3tuir4min5iron_is5hanis3honish5opislan4is4salissen4is4ses",
        "is4ta_ist4lyita4bi4ita5mit3erai5teri4i2ticit3icait5illi4tismi4tramit3uativ3elliv3en_",
        "iv5il_i5vorei4v3ot5izont4jestyk3en4dk3est_kin4delab3iclan4dllan5etlan4tela5tan4lativ",
        "la4v4ald4ereld4erile4mat5lene_lera5b3l4erile5sco5less_li4ato5licioli4cor4lict_lid5er",
        "lif3erli4gra4l4i4llim4bll4im4p1l4inelin3ealiv3erl3le4nl3le4tl2lin4l5linall5outlm3ing",
        "lob5al3logiclom3er5lope_lo5rielor5oulos5etloun5dlp5ingltane5ltera4ltur3al5umn_lus3te",
        "l5vet4mag5inma3ligma5linmal4limal4ty5maniaman5isman3izma5rizmar4lyma5scema3tismba4t5",
        "5mediame3diem5e5dymel5onmem1o3men5acmen4demensu5men4tem5ersa3mestimet3alme5thime3try",
        "3miliam5ineem4inglmis4timma5rymoi5semon5etmon5gemoni3amo3nizmonol4mo3ny_4mora_mo5sey",
        "mpara5mpar5imphas4mp5iesm4p1inmpo3rim4pousmulti32n1a2bna5liana5mitnanci4nan4itnas5ti",
        "na3talnau3sen4ces_n5cheon5chiln3chisn5d2ifne4gatnel5iznera5bn4erarn4er5i3neticn5gere",
        "n3gerini3miz5nine_nis4ta3nition3itorn5keronni3alno3ble4n3o2dnois5ino5l4i3nomicnon4ag",
        "n5oniznor5abnpre4cnsati4n4s3esnter3snti4ern3tinentu3menuf4fe3nu3itoast5eob3a3bob5ing",
        "o3cheto4clamoc3rac5ocritoc3ulao5cureod5dedof5iteofit4to4gatoo5gene1o1giso5g2ly3ogniz",
        "oic3esoi3deroi5letoi5sonoi3terolass4o3lesco3liceol5id_o3li4fol3ingo5lis_ol3isho5lite",
        "olli4eol3umeom3enaom3ic_om3icao5miniomo4geompro53oncilon5eston3keyon4odion3omyonspi4",
        "onten4on3t4iontif53operao5phano5pherop3ingo4posio4r3ago5realore5sh4o5riaor3icaor3ity",
        "or3ougors5enor3thior3thyo3scopos4i4eos3itoos3ityos5tilos5titot3er_ot5ersoth3i4ot3ic_",
        "ot5icao3ticeouch5iover3sov4ertoviti4o5v4olow3derow5est5paganp3agatpan3elpan4typar5di",
        "par5elp4a4ripar4ispa5terpa5thypear4lpedia4ped4icpeli4epe4nanpen4thp4era_p4eragperme5",
        "per3tipe5tenpe5tizphar5iphe3noph4es_ph5ing3phone5phonipi4ciepi5thaplas5tpli5erplum4b",
        "po3et55pointpoly5tppa5rapray4e5precipre5copre3empre4lap3rese3press5pri4epris3op3roca",
        "pros3ept5a4bput3errach4eraf5firam3etrane5oran4gerap3er3raphyrar5ef4rarilra5vairav3el",
        "ra5zier5binerch4err4ci4brdi4errd3ingre5arrre4crere3disred5itre4facreg3isren4tere5pin",
        "re4spire3strre4terre3trire5utire4valrev3elre5vilrg3ingric5as5ricidri4cierid5erri3enc",
        "ri3entrig5anril3iz5rimanrim4pe5rina_riph5erit3icrit5urriv5elriv3etrk4linrl5ishrm5ers",
        "rm3ingr1nis4ro5filro5ker5role_ron4alron4taro3pelrop3icro4therov5elr5pentrp5er_rp3ing",
        "rre4strsa5tirse4crrs5er_rse5v2r4si4brtach4rten4dr4tierrtil3irtil4lr4tilyr4tistru3e4l",
        "rum3plrun4tyruti5nrvel4irv5er_r5vest5ryngesac3risalar4san4desa5vor3s4cie4scopyse2c3o",
        "se4d4ese4molsen5ats5eneds5enin4sentd4sentlsep3a34s1er_4servo5se5umsev3ensh5oldshort5",
        "3side_5sidessi5diz4signa5sine_sion5a3sitiosk5inesk5ingslith5small35smithso4labsol3d2",
        "so3lic3s4on_s5ophyspen4d2s5peo3sphersp5ings5sengs4ses_ssi4erss5ilyssur5astam4i5stand",
        "s4ta4p5stat_s5terostew5a5stickst3ing5stockstom3a5stone3stores4trads4trays4tridsy5rin",
        "tai5lotal5enta5logtan4detanta3ta5perta3riz4taticta4turtax4istch5ettead4ite5gerte2ma2",
        "tem3at3tenan4tenes5ter3dter3ist3ess_teth5eth3easthe5atthe3isth5ic_th5ica5thinkth5ode",
        "ti4atot4ic1utim5ul3tine_ti3zen3tles_t5let_to3natto3rietor5izto3wartra5chtraci4trem5i",
        "4tricstro5mitron5i4tronytro3sptu4nis2t3up_tur3isu4berou3ble_ud5estud3iedud3iesuen4te",
        "uer4ilugh3enuil5izu5lati5ulcheul3derul3ingul5ishul4larul4lisuls5esultra3um4blyumor5o",
        "un5ishunt3abun4tesuper5sup3ingupt5ibure5atur4ferurs5erur5tesur3theur4tieus4linuten4i",
        "4u1t2iu3tineut3ing5u5tiz2v1a4bvac5ilva5lieva5nizvel3liven3omv5enue5vere_v4erelv3eren",
        "v4eresver3ie3versever3thves4tevet3ervi5ali5vide_5vided5vides5vilit4vi4nav3io4rvis3it",
        "vor5ab4voteewa5gerwa5verweath3win4dewo5venwrita4xi5mizxpan4dymbol5yn3chryo5netys3ica",
        "y3thin_semi5a2cablans5granti1dav3iouba1thybe2viebi1orbbio1rhbrus4qchs3hucle4ardata1b",
        "dd5a5bd2d5ibde2mosdes3icde2ticdi2rendi2rerdu2al_e3chasen1dix3fluorge3o1dght1weg1lead",
        "1g2nacgno5mogriev1g1utanhair1shatch1hex2a3hnau3zid4iosign4itjanu3al3chaimedi2cm3ma1b",
        "mu2drond3thrng1sprnk3rupn5lessnon1eqns4moontre1po3chasoerst2oke1stparam4pee2v11pole_",
        "poly1era3chur2amenra3mour3ial_r1kraur1thour1treur1veilsky1scss3hatsta1tist5scrswimm6",
        "tele2gthy3sctot3icus2er_v1ativweek1nxquis3y1stroz3ian_"
      ].join(""),
      "4": [
        "_ch4_ci2_eb4_eg2_es3_eu3_ga2_ge2_he2_in1_le2_me2_od2_os3_sh2_si2_st4_sy2_ta4_te4_th2",
        "_ti2_up3_ye44ab_abe24abr2adi4aduae4raff4ag1iag1na2goa4gya3haa3heah4la3hoa5ia2aleal1i",
        "4alm4amaa2mo4and2angano4a2pla3pu2a2rar1iar2par3q4as_as4la2ta4atha1tra2tua2tyau4bau3r",
        "a2vaav1iaw3iaws4aye4ays45ba_1batba4z2b1bb2be4b1d4be_1bel3betbe3w4b3hbi2bbi4d3bie1bil",
        "bi2tb2l2b4lo4b1m4b3n3bodbo4e3boobt4lb4tob3trbu4n4b5w5by_bys42ce_3cei1cen3cepcet4cew4",
        "4ch_che23ciaci5c4cii2cim2cin5cizck3icly4coe22cogcoi4cov1cri22c1tc2tec4twcud5c4ufc4ui",
        "3cun1cuscze41d2a5da_4daf2dag3dat5dayd1d42de_d4em1dende1pd3eqdes2de1tde1v4dey4d1fd4ga",
        "dg1id2gyd1h25di_3didd1ifd1in1diodir2dis1d5k22dly3do_5doed4ogd4or3dosdo4v3doxdre44dry",
        "ds4pd4swd4syd2thdu2cdu4gdu4n4dup5dynead1ea4lea2tea2v2e1be3bre1ceec2ie1cre1cu4edi4edo",
        "ee2cee2fee2me5ex1effeg5n5egye1h4e5icei5deig2e1lael2fel2iem5be1mee3my4enn4enoe5ofeo2g",
        "e3ole1oreos4e4ote5owe2pae1poer1a2erber1her1i2eroer1s4erues2c4eshe1sie1sp2esses4w4etn",
        "et5ze5une3upeus4e1vie5vue1wae3wh1exp5eyceys44fag5far4f5b4fe_fe4b2fedfer1fev44f1ff2fy",
        "fi3a2finf4l25fonfo2rfos54f5pfri22f3sf4tof2ty4fuggaf42gam4gaz2ge_2ged1gen1geoge4vg2ge",
        "ggo45gi_g1icgil45gio3girgi4u5giv3gizgla41gle3glog4mygn4ag1nig1no3go_gob55goegon25goo",
        "gov1g4rogth3gu4a2gue3gun3gusha4m5hazh4edhe2nhep5h1eshe4theu4hi4phi2vh2lo4h1m2h1nho4g",
        "4h5p4hr4h4shh4tyhu4ghu4thy2s2i1ai2aliam4i2anibe4i1bli5boi1br4ich2iciid5di2dii4dri2du",
        "2ie4i3et4if_i3fl4ift4igii2goi1lail5fil1i4ilnil3vim1ii2mu2in_4ind2ine2ini4ink4inl2inn",
        "2insin1u4iny4io_i1olio4mi4osipe4ip3ii1rair1i2is_4ise3isfi2sois1pi2su2ite2ithi1ti4itt",
        "4i5wix4oizi4ja4pjew3jo4p3ka_k3abk5agkal4k2ed1keeke4gk1erkes45ki_k4imki4pkis44klyko5r",
        "k3ouk4scks4lk4sy4lav2l1blce4l3cil2deldi4l3drle2ales23leyl5frl5galgo32l3h3likl1itl1iz",
        "lka3l2lell2ill4o3lo_4lof4lovl4pll5pr4l1rl4scl2sel1tel1trltu2lu5aluf4lu3o4lup1lut2l1w",
        "4lya4lyb2mab2mah4map4m1b4m5c4me_2medme2gme2m1men2mesme4v4m1f5mi_mi3amig4m2ism2iz4m1l",
        "4m1nmn4amn4o4mokmo2rmos2mo2v4m1pm2pim2py4m3rm4shm5si3mummun24mupmu4unak4n2ann4asn2at",
        "n2aun1crn1cun1de2ne_ne2bne2c2ned1nen3neone2qn1er1nesne4vne4wn1gun2gynha4nhe4ni4dnik4",
        "n1imn1inni4on2it4nk2n1kl4n1lnme4nne43noe4nogno4n4nop1noun1p4npi4nru4ns4cn2sen2slns3m",
        "nt2int4snu1anu4dn4umn3uon1v2n1w4nym4nyp4n3zaoad3o1bio3bro1ceoch4o4elo3ero3evo2fio1ge",
        "o4gl1ogyo1h2oig4o1laol2dol2iol2tol2vo2lyo2meon1aon1c2ondon3soo4ko2pa2opho1prop1uopy5",
        "o1rao1ryos2cos4lo2so4othou4lou5vow1io4wooy1ap4adp4aip4alpa1ppav43pay4p1b4pe_pe2cp4ee",
        "pek4pe2t4ph_ph2l4phsph3t5phu1phypi3ap4idpi2n4p1m2p3npo4cpo4p1posp4ot4p1pp2pep2seps4h",
        "2p1tp2tep2thp4twpub3pue4puf4pu4mpu2n5puspu2t2rabr2air2asrbi2rb4or2cerd2i2re_re1oreu2",
        "rev2rfu4r4fyr1glr3gu4rh_ri3ar4ibri1or4iqr2isrle4r2mer4myrno4r3nur2ocro4erok2rox5r3po",
        "r1r44rs2r1sars4cr2ser1shr1sir1spr5swr1tiru3aru2nrv4er3vory4cry3t5sais1apsau45saw4s5b",
        "sch2s1cu4s5d4se_se4ase2g5sei5sev5sex4s3f2s3g2sh_sho44shwsi1b1siosi2r1sis5siu1siv5siz",
        "4skes1l2s2le2s1ms3mas1n43soms4op4sov2spas1sas1sis4sls4snss2tss5w2st_st2ist4rs2tys4ul",
        "su2msu2nsu2r4swo4syc3syl3ta_2tabta2ltav42taw2t1bt4ch4t1d4te_1teeteg41teote4p3teu3tex",
        "4tey2t1f4t1g2th_th2e4thl2ths1tiatif22tig1tim1tio5tiqti4u1tiv1tizt5lat5lo4t1mtme4to3b",
        "2toftos24t1p1trat4sctsh4t4swt5tottu4tu1atud24tue3tum3tus4two4tya2tyltz4e4uabuac4u1at",
        "uav4ub4eu3cau3cru3cuu4cyud5du4du3ufau3fl2ui2ui4nuiv3u1laul4eul2i4ulsu3luul5vu1mium2p",
        "u2neu1niunu4un5yun5zu5osu1ouu1peu3plup3pu1raurc4ur1dur2lu3ruusc2us1pu2suuts4uu4mu1v2",
        "uxu3uz4e5va_vag43vat4ve_4vedveg3v3ifvik42vilv1invi4p3viv5vo_voi43vok4vv42wacwam4w1er",
        "w3evwhi4wil2wir4wiz5w4no1wo2wom1wra4wri4w3shws4lxam3x4apxas5x3c2x2edxhi2xhu4xi5axi5c",
        "x4obx1t2x3tixu3ay5acy5aty2cey3chy5eey1eryes4ye4ty5gi4y3hy3lay3loy5luyme4yn5dyn5g5ynx",
        "y1o4yo5dyom4y4osyp3iy3poy5puy3royr4rys4cy4soyss4ys1tzar2ze4nze4pz1erzet42z1iz4ilz4is",
        "zo4mzte4z4zy_ev2i4jk6rk_st5bt1crt1wh"
      ].join(""),
      "8": [
        "_chill5i_cor5ner_dictio5_eq5ui5t_for5mer_re5stat_trib5utab5it5abab5o5lizap5illara5ra",
        "meteation5arces5si5bch5a5nisch5inesse4q3ui3sg5rapher5graphicimenta5rin5dlingin5gling",
        "lem5aticl5i5tics5losophyma5chinema5rine_mpos5itenato5mizneg5ativni5ficat5nologisntro",
        "l5lioc5ratizonspir5appo5siterec5omper5ev5er_5taboliz5tisticatrav5es5url5ing__de5riva",
        "_kil2n3i_le6ices_me4ga1l_met4ala_non1e2m_post1am_sem6is4_sph6in1_te3legr_un3at5t_un5",
        "err5_vi2c3aran3ti1n2ar3che5tar2rangeback2er_b1i3tivebo2t1u1lbus6i2erbus6i2esbuss4ing",
        "cat1a1s2croc1o1dcro5e2co2d3alonede5clar13do5worddren1a5ldrif2t1adrom3e5dea4n3iesed1u",
        "lingeli2t1ise3spac6ifit5ted_fla1g6elflow2er_5glo5bingnet1ismg2noresphite3sidho6r1ic_",
        "hypo1thaignit1erim3ped3ainfra1s2i1tesima1ke6lingla4c3i5elar5ce1nlead6er_1le1noidlith",
        "1o5gload4ed_load6er_lth5i2ly1lunk3erman3u1scmar1gin1medio6c1me3gran33mi3da5bmil2l1ag",
        "mi6n3is_mo5e2lasmon4ey1lmo4no1enmoth4et2m1ou3sinm5shack2mul2ti5u4n5i4an_nom5e1non1o1",
        "mistnon1i4soo3mecha6o4s3pherou3ba3dooxi6d1icpar4a1lepi2c1a3dpli2c1ab1prema3cpres2pli",
        "pro2cesspto3mat4pu5bes5cqu6a3si3qui3v4arr1abolicradi1o6gra5n2hasr3bin1gero5e2lasr3pa",
        "u5lisca6p1ersca2t1ol1sci2uttsem1a1phse3mes1tsep3tembsi5resid1s2pacinspi2c1ilspokes5w",
        "sports3csports3ws3qui3tostar3tli1stor1abstrib5ut1syth3i2t6ap6athtch3i1ertele1r6o3ter",
        "1geit3ess2estha4l1amtho1k2erthy4l1an2t3i4an_ti2n3o1mtra1verstreach1etrof4ic_tro1p2is",
        "ua3dratiuad1ratu1verely_v1er1eigves1titewrap3arowrit6er_ym5e5try"
      ].join(""),
      "9": [
        "_ratio5nac5laratioec5ificatef5i5niteep5etitio5losophiz5mocratiz5nop5o5liuto5matic_et",
        "h1y6l1_ga4s1om1_ko6r1te1_mim5i2c1_mi1s4ers_rav5en1oa4pe5ableas5ymptotba6r1onieco6ph1",
        "o3nde2c5lina3di1methydu1op1o1le2r3i4an_5eu2clid1graph5er_ha2p3ar5ri5nitely_irre6v3oc",
        "japan1e2s1ki5neticlai6n3essl3chil6d11lec3ta6bl5mo3nelllo1bot1o11lum5bia_3lyg1a1mimed",
        "3i3cinmil5li5li5maph1ro15moc1ra1tmoro6n5isn3ar4chs_n3ch2es1t5nop1oly_o3les3tero1lo3n",
        "4omor5tivelyo5test1ero5tes3toroth3e1o1so6v3i4an_parag6ra4phi2l3antquain2t1ere2c3i1pr",
        "rec5t6angre4t1riburo5epide1rse1rad1iscrap4er_s2s3i4an_strat1a1gstu1pi4d1su2per1e6tea",
        "ch4er_trai3tor1ttrib1ut1v1oir5du1yes5ter1y"
      ].join(""),
      "3": [
        "a2da2fai2a1ja2n4ao2bfb1jbk44bp2btb1v1cac3c2ch1cick1c5n1coc1q1cyd1bd5cd1jd1m1dod1p1dr",
        "1dud1vd1wd2ye1fei2e1je1q4eu1fa4fd4fh1fi4fm4fn1fo2ft3fu1fy1gag3bgd4gl2g1m1gog3p1grgs2",
        "g3w1gyh1bh1fh1h4hkh1w2id2igi3hi3j4ik2io2ip4iri1u2iv4iy5ja1jek1bk3fkh4k1ikk4k1lk1mk5t",
        "k1w2ld2lf4ljl1l2lm2lp4lt1ly1ma2mh4mkm1m1mo4mt1mu4mw1nan3fn1jn5mn1qn1rn1t4nz4oaoi2o5j",
        "2oko2noo2o1qou21papd44pf4pgpr2p3wqu2r1br1cr1frg2rh4r3jr1lr1mr1pr1wsa2s2hsk21sos1r2ss",
        "1su4svsw2s4y1ta4tc2tl1to1tu4tvtw41ty4tzu5j4uk2usu3u1vav4yw1bwi2w4kw5p4wtwy4x1ax1ex1h",
        "x3ix3ox3px1uxx4y1by1cy1dy1iy1wza14zb2ze5zl4zm1zow3cx1q"
      ].join(""),
      "10": [
        "_ever5si5b_ta5pes1tram1en3ta5bath3er1o1sc2tro3me6c1cu2r1ancede3fin3itid1ri3pleg5h3i5",
        "pel1a4itin5er5armi1n2ut1ernfi6n3ites5noc3er1osoli3gop1o1o3no2t1o3nproc3i3ty_quin5tes",
        "5s3rab1o1loi5sa3par5ilse1mi6t5ic3slova1kia3s2og1a1mys2s1a3chu1s5sign5a3bth1o5gen1i3t",
        "ro1le1umvi1vip3a3rwaste3w6a2"
      ].join(""),
      "12": "1e6p3i3neph13ra4m5e1triz",
      "11": [
        "eth1y6l1ene2g1o4n3i1zale3g6en2dremi1n2ut1estor4tho3ni4tphi5lat1e3lpo3lyph1onopseu3d6",
        "o3d2pseu3d6o3f2schro1ding1s2t1ant5shi5tab1o1lismtravers3a3b3trop1o5les3trop1o5list1r",
        "o1pol3it"
      ].join("")
    },
    exceptions: [
      "as\u2027so\u2027ciate,as\u2027so\u2027ciates,dec\u2027li\u2027na\u2027tion,oblig",
      "\u2027a\u2027tory,phil\u2027an\u2027thropic,present,presents,project,projects,reci",
      "\u2027procity,re\u2027cog\u2027ni\u2027zance,ref\u2027or\u2027ma\u2027tion,ret\u2027",
      "ri\u2027bu\u2027tion,ta\u2027ble"
    ].join("")
  };

  root.Typeset = Typeset;
  root.Hypher = Hypher;
})();

/* Paragraph-wide Knuth-Plass layout for English reading prose, lists, definitions,
   admonitions, footnotes, blockquotes, links, inline code, and inline math.
   The original DOM is retained for resize and print. Inline math, footnote/citation
   references, keyboard badges, superscripts, subscripts, and inline target spans are
   measured as unbreakable boxes and moved (not cloned) into their lines. */
(() => {
  "use strict";
  const article = document.querySelector(".gd-article");
  const language = (document.documentElement.lang || "en").toLowerCase();
  if (
    !article ||
    !/^en(?:-|$)/.test(language) ||
    typeof Typeset === "undefined" ||
    typeof Typeset.linebreak !== "function" ||
    !Typeset.LinkedList ||
    typeof Hypher === "undefined" ||
    !Hypher.en
  ) {
    return;
  }
  const hypher = new Hypher(Hypher.en);
  const breaker = Typeset.linebreak;
  const canvas = document.createElement("canvas").getContext("2d");
  if (!canvas) return;
  const narrowMedia = typeof matchMedia === "function"
    ? matchMedia("(max-width: 39.99rem)")
    : null;
  const originals = new Map();
  // Elements set as one unbreakable box. Each stands for one OBJECT REPLACEMENT CHARACTER
  // in the text and is moved, not copied, into the line that holds it.
  const atoms = [
    ".math",
    "mjx-container",
    "a.footnote-reference",
    "a.citation-reference",
    "kbd",
    "span.guilabel",
    "sup",
    "sub",
    "span[id]:empty"
  ].join(", ");
  const ATOM = "\uFFFC";
  const inlineTag = /^(A|EM|STRONG|SPAN|CODE|CITE|ABBR|DEL|INS|MARK|DFN|VAR|SAMP|SMALL|B|I)$/;
  const blockDisplay = /^(block|flex|grid|table|list-item)$/;
  const selector = [
    "section > p",
    ".desc-content > p",
    "li > p",
    "dd > p",
    "li",
    "dd",
    ".admonition > p:not(.admonition-title)",
    "blockquote > p:not(.attribution)",
    "aside.footnote > p:not(.label)",
    "aside.citation > p:not(.label)",
    "aside.topic > p",
    "aside.sidebar > p",
    "p.lead",
    ".gd-abstract"
  ].join(", ");
  const excluded = [
    "table",
    ".toctree-wrapper",
    ".postlist",
    ".postlist-cards",
    "figure",
    "figcaption",
    "nav",
    ".docinfo",
    ".gd-article-meta",
    ".gd-download",
    ".gd-actions",
    ".gd-tagline",
    ".gd-lead",
    ".gd-flow",
    ".gd-flow-caption",
    ".gd-features",
    ".gd-editions",
    ".field-list",
    ".option-list",
    ".hlist",
    ".line-block",
    ".rubric",
    ".attribution",
    ".label"
  ].join(", ");

  const outsideAtoms = p => (
    [...p.querySelectorAll("*")].filter(e => !e.parentElement.closest(atoms))
  );
  const paragraphs = [...article.querySelectorAll(selector)].filter(p => {
    const length = p.textContent.length;
    return (
      length >= 65 &&
      length <= 5000 &&
      !p.matches(excluded) &&
      !p.closest(excluded) &&
      outsideAtoms(p).every(e => (
        e.matches(atoms) ||
        (inlineTag.test(e.tagName) &&
         !e.matches("[id], [role], [tabindex]") &&
         !blockDisplay.test(getComputedStyle(e).display))
      ))
    );
  });

  // Atoms moved into the lines of each paragraph, with the copies left in their place.
  const swaps = new Map();

  function restore(p) {
    for (const [atom, copy] of swaps.get(p) || []) copy.replaceWith(atom);
    swaps.delete(p);
    const original = originals.get(p);
    if (original) p.replaceChildren(...original);
    delete p.dataset.layout;
  }

  function textMap(p) {
    const filter = NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT;
    const walker = document.createTreeWalker(p, filter, {
      acceptNode: node => node.parentElement.closest(atoms)
        ? NodeFilter.FILTER_REJECT
        : NodeFilter.FILTER_ACCEPT
    });
    const map = [];
    let text = "", node;
    while ((node = walker.nextNode())) {
      if (node.nodeType === Node.TEXT_NODE) {
        map.push({
          node,
          element: node.parentElement,
          start: text.length,
          end: text.length + node.length
        });
        text += node.textContent;
      } else if (node.matches(atoms)) {
        map.push({
          atom: node,
          element: node.parentElement,
          start: text.length,
          end: text.length + 1,
          width: node.getBoundingClientRect().width
        });
        text += ATOM;
      }
    }
    return { map, text };
  }

  function itemAt(map, offset) {
    return map.find(a => offset >= a.start && offset < a.end) || map.at(-1);
  }

  function point(map, offset) {
    const item = itemAt(map, offset);
    if (item.atom) {
      const parent = item.atom.parentNode;
      const index = [...parent.childNodes].indexOf(item.atom);
      return { node: parent, offset: index + (offset > item.start ? 1 : 0) };
    }
    return { node: item.node, offset: Math.min(offset - item.start, item.node.length) };
  }

  function fontOf(item) {
    if (!item.font) {
      const s = getComputedStyle(item.element);
      item.font = `${s.fontStyle} ${s.fontWeight} ${s.fontSize} ${s.fontFamily}`;
    }
    return item.font;
  }

  function fontAt(map, offset) {
    return fontOf(itemAt(map, offset));
  }

  function width(text, font) {
    canvas.font = font;
    return canvas.measureText(text).width;
  }

  function spanWidth(map, offset, length) {
    let total = 0;
    for (let at = offset, end = offset + length; at < end;) {
      const item = itemAt(map, at), stop = Math.min(end, item.end);
      // Past the end of the text the last item cannot advance us; stop rather than spin.
      if (stop <= at) break;
      total += item.atom
        ? item.width
        : width(item.node.data.slice(at - item.start, stop - item.start), fontOf(item));
      at = stop;
    }
    return total;
  }

  function codesIn(map, start, end) {
    return new Set(
      map
        .filter(item => item.end > start && item.start < end)
        .map(item => item.element.closest("code"))
        .filter(Boolean)
    );
  }

  function codeEdges(map, start, end) {
    let left = 0, right = 0;
    for (const code of codesIn(map, start, end)) {
      const items = map.filter(item => item.element.closest("code") === code);
      const style = getComputedStyle(code);
      if (items[0].start >= start) {
        left += (parseFloat(style.paddingLeft) || 0) +
          (parseFloat(style.borderLeftWidth) || 0);
      }
      if (items.at(-1).end <= end) {
        right += (parseFloat(style.paddingRight) || 0) +
          (parseFloat(style.borderRightWidth) || 0);
      }
    }
    return [left, right];
  }

  const wordPattern = new RegExp(
    "^([\\uFFFC\\u201C\\u2018(]*)(\\p{L}{6,})" +
    "((?:['\\u2019]\\p{L}+)?[.,;:!?\"\\u201D\\u2019)\\]\\uFFFC]*-?)$",
    "u"
  );

  function pushSyllables(map, part, partStart, inCode, font, extra, nodes) {
    const letters = inCode ? null : part.match(wordPattern);
    // Build a new array: the hyphenator's result must not carry this word's punctuation
    // into the next occurrence of the same word.
    const fragments = letters ? [...hypher.hyphenate(letters[2])] : [part];
    if (letters) {
      fragments[0] = letters[1] + fragments[0];
      fragments[fragments.length - 1] += letters[3];
    }
    let offset = partStart;
    for (let j = 0; j < fragments.length; j++) {
      const fragment = fragments[j];
      const boxWidth = spanWidth(map, offset, fragment.length) + (j === 0 ? extra : 0);
      nodes.push(breaker.box(boxWidth, fragment));
      offset += fragment.length;
      if (j < fragments.length - 1) {
        const penalty = breaker.penalty(width("‐", font), 80, 1);
        penalty.offset = offset;
        penalty.trimEnd = offset;
        nodes.push(penalty);
      }
    }
  }

  const codeSplit =
    /(?<=\/)(?=[^/\s])|(?<=::)(?=.)|(?<=[a-zA-Z0-9]{3}[._-])(?=[a-zA-Z0-9]{3})/;
  const proseSplit = /(?<=\p{L}-)(?=\p{L})|(?<=\/)(?=[^/\s])/u;

  function nodesFor(text, map, space) {
    const nodes = [];
    const words = [...text.matchAll(/\S+/g)];
    for (let i = 0; i < words.length; i++) {
      const word = words[i][0], start = words[i].index, end = start + word.length;
      const inCode = codesIn(map, start, end).size > 0;
      const font = fontAt(map, start);
      const [left, right] = inCode ? codeEdges(map, start, end) : [0, 0];
      const parts = word.split(inCode ? codeSplit : proseSplit);
      let partOffset = start;
      for (let p = 0; p < parts.length; p++) {
        const part = parts[p];
        const extra = (p === 0 ? left : 0) + (p === parts.length - 1 ? right : 0);
        pushSyllables(map, part, partOffset, inCode, font, extra, nodes);
        partOffset += part.length;
        if (p < parts.length - 1 && inCode) {
          const preferred = part.endsWith("/") || part.endsWith("::");
          const penalty = breaker.penalty(0, preferred ? 100 : 220, 0);
          penalty.offset = partOffset;
          penalty.trimEnd = partOffset;
          nodes.push(penalty);
        } else if (p < parts.length - 1) {
          const isHyphen = part.endsWith("-");
          const penalty = breaker.penalty(0, isHyphen ? 50 : 100, isHyphen ? 1 : 0);
          penalty.offset = partOffset;
          penalty.trimEnd = partOffset;
          nodes.push(penalty);
        }
      }
      const last = i === words.length - 1;
      const codeSpace = !last && codesIn(map, end, end + 1).size > 0;
      const gap = codeSpace ? width(" ", fontAt(map, end)) : space;
      const glue = breaker.glue(
        last ? 0 : gap,
        last ? breaker.infinity : space / 2,
        last ? 0 : space / 3
      );
      glue.offset = last ? text.length : words[i + 1].index;
      glue.trimEnd = end;
      nodes.push(glue);
    }
    const final = breaker.penalty(0, -breaker.infinity, 0);
    final.offset = text.length;
    final.trimEnd = text.length;
    nodes.push(final);
    return nodes;
  }

  function lineFragment(map, start, end) {
    const range = document.createRange();
    const from = point(map, start), to = point(map, end);
    range.setStart(from.node, from.offset);
    range.setEnd(to.node, to.offset);
    return range.cloneContents();
  }

  function moveAtoms(map, fragment) {
    const placed = [];
    const copies = [...fragment.querySelectorAll(atoms)].filter(
      e => !e.parentElement.closest(atoms)
    );
    const items = map.filter(item => item.atom);
    if (copies.length !== items.length) return placed;
    copies.forEach((copy, i) => {
      const atom = items[i].atom, marker = document.createComment("");
      copy.replaceWith(marker);
      atom.replaceWith(copy);
      marker.replaceWith(atom);
      placed.push([atom, copy]);
    });
    return placed;
  }

  function render(p, map, text, nodes, breaks, space) {
    const fragment = document.createDocumentFragment();
    const codeOf = offset => (
      offset >= 0 && offset < text.length
        ? itemAt(map, offset).element.closest("code")
        : null
    );
    let start = 0, cut = false;
    for (let i = 1; i < breaks.length; i++) {
      const choice = breaks[i], node = nodes[choice.position];
      const trimEnd = node.trimEnd ?? node.offset;
      const line = document.createElement("span");
      line.className = "gd-typeset-line";
      line.append(lineFragment(map, start, trimEnd));
      if (node.type === "penalty" && node.penalty > 0 && node.width > 0) {
        line.dataset.hyphen = "";
      }
      if (cut) line.querySelector("code")?.classList.add("gd-code-cut-start");
      cut = codeOf(trimEnd - 1) !== null && codeOf(trimEnd - 1) === codeOf(node.offset);
      if (cut) [...line.querySelectorAll("code")].at(-1)?.classList.add("gd-code-cut-end");
      if (i < breaks.length - 1 && choice.ratio < 0) {
        line.style.wordSpacing = `${choice.ratio * space / 3}px`;
      }
      if (trimEnd < node.offset) {
        const sep = document.createElement("span");
        sep.className = "gd-line-space";
        sep.textContent = text.slice(trimEnd, node.offset);
        line.append(sep);
      }
      fragment.append(line);
      start = node.offset;
    }
    if (start !== text.length || fragment.textContent !== p.textContent) return;
    if (!originals.has(p)) originals.set(p, [...p.childNodes]);
    const placed = moveAtoms(map, fragment);
    if (placed.length !== map.filter(item => item.atom).length) return;
    p.replaceChildren(fragment);
    swaps.set(p, placed);
    p.dataset.layout = "knuth-plass";
  }

  // Measures one restored paragraph and chooses its breaks without changing the page;
  // returns null when the paragraph keeps the browser's layout.
  function plan(p) {
    const style = getComputedStyle(p);
    const align = style.textAlign;
    if (
      style.display === "flex" ||
      style.display === "grid" ||
      align === "center" ||
      align === "right" ||
      align === "end"
    ) {
      return null;
    }
    const measure = p.clientWidth;
    if (measure < 380) return null;
    const { map, text } = textMap(p);
    if (!map.length || !text.trim()) return null;
    const space = width(" ", fontOf({ element: p }));
    const nodes = nodesFor(text, map, space);
    if (nodes.length > 1800) return null;
    const breaks = breaker(nodes, [measure - 1], {
      tolerance: 2,
      maxTolerance: 4,
      demerits: { line: 10, flagged: 400, fitness: 3000 }
    });
    if (breaks.length < 3 || breaks.at(-1).position !== nodes.length - 1) return null;
    if (breaks.slice(1, -1).some(b => b.ratio > 2.5 || b.ratio < -0.95)) return null;
    return { p, map, text, nodes, breaks, space };
  }

  let scheduled = false, printing = false;
  // Restore every paragraph, then measure them all, then rewrite them all. Interleaving
  // measurement with rewriting made the browser lay out the whole page once per
  // paragraph, which froze long pages for many seconds.
  function run() {
    scheduled = false;
    if (printing) return;
    for (const p of paragraphs) restore(p);
    if (narrowMedia?.matches) return;
    const plans = paragraphs.map(plan).filter(Boolean);
    for (const { p, map, text, nodes, breaks, space } of plans) {
      render(p, map, text, nodes, breaks, space);
    }
  }
  function schedule() {
    if (scheduled || printing) return;
    scheduled = true;
    requestAnimationFrame(run);
  }
  // MathJax is listed after this script, so while it runs window.MathJax may hold only
  // the configuration. Then MathJax itself has not loaded, and we wait for the page to
  // load before waiting for its first typesetting.
  function mathTypeset() {
    const mathJax = window.MathJax;
    if (!mathJax || mathJax.startup?.promise) return mathJax?.startup?.promise;
    const page = document.readyState === "complete"
      ? Promise.resolve()
      : new Promise(resolve => addEventListener("load", resolve, { once: true }));
    return page.then(() => window.MathJax?.startup?.promise);
  }
  const loaded = document.readyState === "loading"
    ? new Promise(resolve => addEventListener("DOMContentLoaded", resolve, { once: true }))
    : Promise.resolve();
  loaded
    .then(mathTypeset)
    .catch(() => {})
    .then(() => document.fonts.ready)
    .then(() => {
      run();
      let previousWidth = 0;
      const resized = entries => {
        const measure = entries[0].contentRect.width;
        if (Math.abs(measure - previousWidth) > 0.5) {
          previousWidth = measure;
          schedule();
        }
      };
      if (typeof ResizeObserver !== "undefined") {
        new ResizeObserver(resized).observe(article);
      } else {
        addEventListener("resize", schedule);
      }
      narrowMedia?.addEventListener?.("change", schedule);
    });
  addEventListener("beforeprint", () => {
    printing = true;
    for (const p of paragraphs) restore(p);
  });
  addEventListener("afterprint", () => {
    printing = false;
    schedule();
  });

  const blockTags = /^(P|LI|DD|DT|H1|H2|H3|H4|H5|H6|PRE|BLOCKQUOTE|DIV|SECTION|ASIDE|TR)$/;
  function plainText(rootNode, inPre = false) {
    let out = "";
    for (const child of rootNode.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) {
        out += inPre ? child.data : child.data.replace(/[ \t]*\n[ \t]*/g, " ");
      } else if (child.nodeType === Node.ELEMENT_NODE) {
        if (child.tagName === "BR") {
          out += "\n";
        } else if (blockTags.test(child.tagName)) {
          if (out && !out.endsWith("\n")) out += "\n";
          out += plainText(child, inPre || child.tagName === "PRE");
          if (!out.endsWith("\n")) {
            out += child.tagName === "LI" || child.tagName === "TR" ? "\n" : "\n\n";
          }
        } else {
          out += plainText(child, inPre);
        }
      }
    }
    return out;
  }

  // Copy the selected source text, without visual line divisions or inserted hyphens.
  addEventListener("copy", event => {
    const selection = getSelection();
    if (!selection || selection.isCollapsed || !selection.rangeCount || !event.clipboardData) {
      return;
    }
    const inTypeset = node => (
      (node.nodeType === Node.TEXT_NODE ? node.parentElement : node)
        ?.closest('[data-layout="knuth-plass"]')
    );
    const from = inTypeset(selection.anchorNode), to = inTypeset(selection.focusNode);
    const fragment = selection.getRangeAt(0).cloneContents();
    if (from && from === to) {
      const cleaned = fragment.textContent.replace(/[ \t]*\n[ \t]*/g, " ");
      event.clipboardData.setData("text/plain", cleaned);
      event.preventDefault();
      return;
    }
    if (!fragment.querySelector(".gd-typeset-line")) return;
    for (const span of [...fragment.querySelectorAll(".gd-typeset-line, .gd-line-space")]) {
      span.replaceWith(...span.childNodes);
    }
    const cleaned = plainText(fragment).replace(/\n{3,}/g, "\n\n").trimEnd();
    event.clipboardData.setData("text/plain", cleaned);
    event.preventDefault();
  });
})();
