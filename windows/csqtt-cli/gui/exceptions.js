/**
 * Exceptions and Routing Screen for csqtt-win (Material 3)
 *
 * Implements the Exceptions/Routing tab mirroring Android ExceptionsTab.kt,
 * with Windows adaptations for per-app executable list editing.
 *
 * Includes:
 * - App routing card and modal dialog (per-process exe name / path exclusion with validation)
 * - Domain routing card and modal dialog (blacklist/whitelist, template RU domains, syntax validation)
 * - IP routing card and modal dialog (CIDR/ranges/IPs validation)
 * - Rules transfer card and modal dialog (version 2 JSON & plain text import/export via clipboard)
 * - Server routing active indicator
 */

(function () {
  "use strict";

  // 503 curated RU domains mirroring Android assets/domains.txt
  var TEMPLATE_DOMAINS_RAW =
    "ya.ru yandex.com yandex.net yandex.org yandex.by yandex.kz yandex.uz yandex.tr yandex.eu yandex.fr " +
    "yandex.de yandex.ee yandex.lt yandex.lv yandex.md yandex.pl yandex.tj yandex.tm yandex.ua yandex.sx " +
    "yandex.jobs yandex.aero yandex.cloud yandex.co.il yandex.com.am yandex.com.ge yandex.com.ru yandex.com.tr " +
    "yandex.com.ua yandexadexchange.net yandexcloud.net yandexcom.net yandexmetrica.com yandexwebcache.net " +
    "yandexwebcache.org yastat.net yastatic.net yastatic-net.ru yads.tech vk.com vk.me vk.cc vk.design vk.link " +
    "vk.team vk.company vk-apps.com vk-cdn.me vk-cdn.net vk-portal.net vkcache.com vkcloud-static.ru vkgo.app " +
    "vklive.app vkmessenger.app vkmessenger.com vkuser.net vkuseraudio.com vkuseraudio.net vkuseraudio.ru " +
    "vkusercdn.ru vkuserlive.net vkuserphoto.ru vkuservideo.com vkuservideo.net vkuservideo.ru vkvideo.ru " +
    "vkontakte.ru ok.ru okcdn.ru okko.sport okko.tv mycdn.me mradx.net mvk.com imgsmail.ru mail.ru avito.ru " +
    "avito.st wildberries.ru wb.ru ozon.ru ozonusercontent.com ozone.ru dns-shop.ru magnit.ru 5ka.ru " +
    "perekrestok.ru chizhik.club dixy.ru lenta.com lenta.ru vkusvill.ru fix-price.com edadeal.ru edadeal.io " +
    "kuper.ru sbermarket.ru megamarket.ru megamarket.tech sbermegamarket.ru topdelivery.ru dellin.ru " +
    "bronevik.com rostaxi.org taxsee.com taximaxim.ru citydrive.ru whoosh.bike rzd.ru rzd-bonus.ru tutu.ru " +
    "tu-tu.ru auto.ru av.ru hh.ru kinopoisk.ru rutube.ru rutubelist.ru dzen.ru dzeninfra.ru gazeta.ru kp.ru " +
    "rbc.ru kommersant.ru ria.ru rg.ru aif.ru forbes.ru gismeteo.ru rambler.ru rambler-co.ru iz.ru " +
    "tnt-online.ru vgtrk.ru 1tv.ru ntv.ru smotrim.ru max.ru tamtam.chat beeline.ru megafon.ru mts.ru " +
    "tele2.ru yota.ru rostelecom.ru dom.ru t2.ru mymts.ru rt.ru motivtelecom.ru gosuslugi.ru government.ru " +
    "kremlin.ru nalog.ru cbr.ru mos.ru mosreg.ru mil.ru edu.ru cikrf.ru izbirkom.ru zdrav10.ru ebs.ru " +
    "goskey.ru finuslugi.ru nbki.ru moex.com domclick.ru alfabank.ru alfabank.com alfa-bank.ru alfa-bank.com " +
    "tbank-online.com cdn-tinkoff.ru tinkoff.ru tinkoff.com sberbank.ru sber.ru vtb.ru vtb24.ru gazprombank.ru " +
    "rshb.ru psbank.ru open.ru raiffeisen.ru homecredit.ru sovcombank.ru kaspersky.ru kaspersky.com drweb.ru " +
    "1c-bitrix.ru bitrix24.ru kontur.ru kontur.host reg.ru timeweb.com timeweb.cloud fastvps.ru 1cloud.ru " +
    "usedesk.ru jivo.ru jivochat.com jivosite.com habr.com drom.ru drive2.ru banki.ru sravni.ru t-j.ru " +
    "ruwiki.ru rustore.ru tns-counter.ru webvisor.com webvisor.org ads.x5.ru x5.ru x5.tech x5club.ru x5id.ru " +
    "x5l.ru x5paket.ru x5q.ru 5post.market fivepost.ru pochta.ru dnevnik.ru emias.info gorodpay.ru " +
    "mirpayonline.ru nspk.ru sbp.ru donationalerts.com boosty.to okolo.app myapelsin.ru netmonet.co " +
    "lead-pro2023.online miya-news.online hematonix.ru medtrum.ru medtrum.eu freestylediabetes.ru " +
    "ican-sinocare.ru lumiflex.ru marine.ru i-ola.ru qms.ru grfc.ru csat.ru cscampus.ru res-nsdi.ru " +
    "auth-nsdi.ru clstorage.net static-storage.net trbcdn.net rtbcdn.ru naydex.net rsscenter.cloud " +
    "dbankcloud.ru lmru.tech tsx.x5static.net youla.ru youla.io youla-web-static.mrgcdn.ru kazanexpress.ru " +
    "e5.ru oneme.ru api.oneme.ru fd.oneme.ru i.oneme.ru sdk-api.apptracer.ru uxfeedback.ru tildaapi.com " +
    "turbopages.org miniapps.max.ru st.max.ru tracker-api.vk-analytics.ru reviews.2gis.com 2gis.com 2gis.ru " +
    "2gis.ua 2gis.by 2gis.kz 2gis.uz 2gis.tj 2gis.az 2gis.am 2gis.ge 2gis.kg 2gis.cz 2gis.com.cy 2gis.ae " +
    "secf1rst.shop 1c.ru 1cfresh.com 1internet.tv 2ip.ru 47news.ru 4meeting.me abr.ru aclub.ru adfox.ru " +
    "admetrica.ru adygeya.ru aeroflot.ru alfa-finance.com alfa-fx.com alfa-pc.com alfa-usa.com alfafinance.biz " +
    "alfafinance.ru alfafuture.com alfafuture.ru alfafx.com alfaleasing.ru alfaprivate.com alformacap.com " +
    "alformacapital.com altai.ru amur.ru anytimeru.com arkhangelsk.ru astrakhan.ru baltbank.ru banka-ui.dev " +
    "bankline.ru bashkiria.ru belgorod.ru beta-bank.com bir.ru bryansk.ru buryatia.ru cbg.ru cdn-vk.ru " +
    "checkip.amazonaws.com chel.ru chelyabinsk.ru chita.ru chukotka.ru chuvashia.ru credistory.ru crimea.ru " +
    "dagestan.ru dbo-dengi.online dodopizza.ru ekburg.ru gazprombank.tech gazprompay.ru gov.ru gpb.ru " +
    "gpmdi.ru grozny.ru gu-st.ru idx5.ru ifconfig.me investalfabank.com ip.sb ipapi.is ipify.org iplocate.io " +
    "irk.ru irkutsk.ru ivanovo.ru izhevsk.ru jamal.ru jar.ru jx5.ru kalmykia.ru kaluga.ru kamchatka.ru " +
    "karelia.ru kazan.ru kchr.ru kemerovo.ru khabarovsk.ru khakassia.ru khv.ru kirov.ru koenig.ru " +
    "kostroma.ru krasnodar.su krasnoyarsk.ru krasyar.ru krd.ru kuban.ru kurgan.ru kursk.ru lemanapro.ru " +
    "lipetsk.ru magadan.ru mari-el.ru mari.ru memealerts.com mgfoms.ru mm.ru mnogolososya.ru mordovia.ru " +
    "mtsdengi.ru murmansk.ru nalchik.ru nn.ru nov.ru novosibirsk.ru nsk.ru omsk.ru orb.ru oryol.ru penza.ru " +
    "perm.ru psblog.ru psk.ru pskov.ru ptz.ru rkomi.ru rnd.ru russiacalling.com ryazan.ru sakhalin.ru " +
    "samara.ru saratov.ru sbpgpb.ru sev.ru sevastopol.ru simbirsk.ru sistema-capital.com smolensk.ru spb.ru " +
    "spvb.ru stavropol.ru stv.ru surgut.ru svoy.academy tambov.ru tatarstan.ru tips.tips tochka-tech.com " +
    "tochka.com tom.ru tomsk.ru tsaritsyn.ru tsk.ru tula.ru tuva.ru tver.ru tyumen.ru udm.ru udmurtia.ru " +
    "ulan-ude.ru userapi.com victoria-group.ru vk-analytics.ru vk-apps.ru vk.ru vkusnoitochka.ru vl.ru " +
    "vladikavkaz.ru vladimir.ru vladivostok.ru vlg.ru volgograd.ru vologda.ru voronezh.ru vrn.ru " +
    "vtb-grants.fut.ru vtb-liga.fut.ru vtb-russia.com vtb.bank.in vtb.com vtb.corp.ru vtb.digital vtb.fut.ru " +
    "vtb.promo vtb24.com vtbcareer.com vtbfamily.ru vtbindia.com vtbkep.site vtbpartners.com vtbrussia.com " +
    "vtbrussia.ru vtbstrana.ru vyatka.ru windsurf.com wink.ru yakutia.ru yamal.ru yandex-bank.net " +
    "yandex-team.ru yandex.az yandex.fi yandex.kg yandex.ru yandex.st yaroslavl.ru yuzhno-sakhalinsk.ru " +
    "zentotem.net burgerkingrus.ru flypobeda.ru pervye.ru sirena-travel.ru tank-online.com trvl.yandex.net";

  var TEMPLATE_DOMAINS = TEMPLATE_DOMAINS_RAW.split(" ");
  var TEMPLATE_DOMAINS_SET = {};
  for (var i = 0; i < TEMPLATE_DOMAINS.length; i++) {
    TEMPLATE_DOMAINS_SET[TEMPLATE_DOMAINS[i]] = true;
  }

  // --- Domain Normalization & Validation (matching ExceptionsTab.kt) ---

  var DOMAIN_LABEL_REGEX = /^[a-z0-9\u0400-\u04ff]([a-z0-9\u0400-\u04ff\-]{0,61}[a-z0-9\u0400-\u04ff])?$/;

  function cleanDomain(raw) {
    if (!raw) return "";
    var s = String(raw).trim().toLowerCase();
    if (s.indexOf("https://") === 0) s = s.slice(8);
    else if (s.indexOf("http://") === 0) s = s.slice(7);
    if (s.indexOf("www.") === 0) s = s.slice(4);

    var slashIdx = s.indexOf("/");
    if (slashIdx !== -1) s = s.slice(0, slashIdx);
    var colonIdx = s.indexOf(":");
    if (colonIdx !== -1) s = s.slice(0, colonIdx);
    var qIdx = s.indexOf("?");
    if (qIdx !== -1) s = s.slice(0, qIdx);
    var hashIdx = s.indexOf("#");
    if (hashIdx !== -1) s = s.slice(0, hashIdx);

    s = s.replace(/^[,;\s.\(\)\[\]"']+|[,;\s.\(\)\[\]"']+$/g, "");
    return s;
  }

  function isValidDomain(domain) {
    if (!domain || domain.length < 3 || domain.length > 253) return false;
    var parts = domain.split(".");
    if (parts.length < 2) return false;
    var tld = parts[parts.length - 1];
    if (tld.length < 2) return false;
    if (!/[a-z\u0400-\u04ff]/.test(tld)) return false;
    for (var j = 0; j < parts.length; j++) {
      if (!DOMAIN_LABEL_REGEX.test(parts[j])) return false;
    }
    return true;
  }

  function parseDomainsFromText(text) {
    if (!text) return [];
    var tokens = String(text).split(/[,;\s\n\r]+/);
    var seen = {};
    var result = [];
    for (var j = 0; j < tokens.length; j++) {
      var d = cleanDomain(tokens[j]);
      if (isValidDomain(d) && !seen[d]) {
        seen[d] = true;
        result.push(d);
        if (result.length >= 1024) break;
      }
    }
    return result;
  }

  // --- IP Normalization & Validation (matching ExceptionsTab.kt) ---

  function isValidIpv4(ip) {
    if (!ip) return false;
    var parts = String(ip).trim().split(".");
    if (parts.length !== 4) return false;
    for (var j = 0; j < 4; j++) {
      var p = parts[j];
      if (!/^\d+$/.test(p)) return false;
      var num = Number(p);
      if (num < 0 || num > 255) return false;
      if (p !== "0" && p.charAt(0) === "0") return false;
    }
    return true;
  }

  function isValidIpRule(raw) {
    var item = (raw || "").trim();
    if (!item) return false;
    if (item.indexOf("/") !== -1) {
      var slashParts = item.split("/");
      if (slashParts.length !== 2) return false;
      if (!/^\d+$/.test(slashParts[1].trim())) return false;
      var prefix = Number(slashParts[1].trim());
      return prefix >= 0 && prefix <= 32 && isValidIpv4(slashParts[0].trim());
    }
    if (item.indexOf("-") !== -1) {
      var rangeParts = item.split("-");
      if (rangeParts.length !== 2) return false;
      return isValidIpv4(rangeParts[0].trim()) && isValidIpv4(rangeParts[1].trim());
    }
    return isValidIpv4(item);
  }

  function cleanIpRule(raw) {
    var item = (raw || "").trim();
    if (item.indexOf("-") !== -1) {
      var parts = item.split("-");
      if (parts.length === 2) {
        return parts[0].trim() + "-" + parts[1].trim();
      }
    }
    return item;
  }

  function parseIpsFromText(text) {
    if (!text) return [];
    var tokens = String(text).split(/[,;\s\n\r]+/);
    var seen = {};
    var result = [];
    for (var j = 0; j < tokens.length; j++) {
      var r = cleanIpRule(tokens[j]);
      if (isValidIpRule(r) && !seen[r]) {
        seen[r] = true;
        result.push(r);
        if (result.length >= 1024) break;
      }
    }
    return result;
  }

  // --- Windows Apps Normalization & Validation ---

  function cleanAppName(raw) {
    if (!raw) return "";
    var s = String(raw).trim();
    // Strip wrapping quotes
    s = s.replace(/^["']|["']$/g, "").trim();
    return s;
  }

  function parseAppsList(raw) {
    if (!raw) return [];
    return String(raw)
      .split(/[,|\n\r]+/)
      .map(cleanAppName)
      .filter(function (s) { return s.length > 0; });
  }

  function formatAppsList(arr) {
    if (!Array.isArray(arr)) return "";
    return arr.map(cleanAppName).filter(function (s) { return s.length > 0; }).join(",");
  }

  // --- Clipboard Helpers ---

  function copyToClipboard(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text);
    }
    return new Promise(function (resolve, reject) {
      try {
        var ta = document.createElement("textarea");
        ta.value = text;
        ta.style.position = "fixed";
        ta.style.opacity = "0";
        document.body.appendChild(ta);
        ta.focus();
        ta.select();
        var ok = document.execCommand("copy");
        document.body.removeChild(ta);
        if (ok) resolve();
        else reject(new Error("execCommand copy failed"));
      } catch (err) {
        reject(err);
      }
    });
  }

  function readFromClipboard() {
    if (navigator.clipboard && navigator.clipboard.readText) {
      return navigator.clipboard.readText();
    }
    return Promise.reject(new Error("Clipboard API read not supported"));
  }

  // --- Dialog: Apps Management (Windows adaptation) ---

  // Running-process picker: search box + list; a click adds the full path (or the name).
  function showProcessPicker(onPick) {
    var box = window.UI.h("div", { style: { display: "flex", flexDirection: "column", gap: "8px", width: "100%" } });
    var search = window.UI.h("input", {
      className: "m3-field-input",
      attrs: { type: "text", placeholder: "Поиск процесса", autocomplete: "off", spellcheck: "false" },
    });
    var searchWrap = window.UI.h("div", { className: "m3-field-container", style: { height: "44px" } }, [search]);
    var list = window.UI.h("div", { style: { display: "flex", flexDirection: "column", gap: "4px", maxHeight: "340px", overflowY: "auto" } });
    box.appendChild(searchWrap); box.appendChild(list);
    var all = [];
    var dlg;
    function render() {
      var q = search.value.trim().toLowerCase();
      list.innerHTML = "";
      var shown = all.filter(function (p) { return !q || p.name.toLowerCase().indexOf(q) >= 0 || p.path.toLowerCase().indexOf(q) >= 0; });
      if (shown.length === 0) {
        list.appendChild(window.UI.h("div", { className: "body-medium text-muted", text: all.length ? "Ничего не найдено" : "Загрузка…" }));
        return;
      }
      shown.forEach(function (p) {
        var row = window.UI.h("button", {
          className: "m3-button text",
          attrs: { type: "button", title: p.path || p.name },
          style: { justifyContent: "flex-start", textAlign: "left", height: "auto", padding: "6px 10px", flexDirection: "column", alignItems: "flex-start" }
        }, [
          window.UI.h("span", { className: "body-medium", text: p.name }),
          window.UI.h("span", { className: "label-small text-muted", text: p.path || "путь недоступен — будет добавлено имя", style: { wordBreak: "break-all" } })
        ]);
        row.addEventListener("click", function () { onPick(p.path || p.name); if (dlg) dlg.close(); });
        list.appendChild(row);
      });
    }
    search.addEventListener("input", render);
    dlg = window.UI.dialog({
      title: "Запущенные программы",
      content: box,
      actions: [{ text: "Закрыть", variant: "text", onClick: function (d) { d.close(); } }]
    });
    render();
    window.csqtt.listProcesses().then(function (r) { all = Array.isArray(r) ? r : []; if (!all.length) all = []; render(); })
      .catch(function (e) { list.innerHTML = ""; list.appendChild(window.UI.h("div", { className: "body-medium", text: "Ошибка: " + (e && e.message || e) })); });
  }

  function showAppsDialog() {
    var p = window.App.profile() || {};
    var isWhite = !!p.is_whitelist;

    var blackList = parseAppsList(p.excluded_apps);
    var whiteList = parseAppsList(p.whitelist_apps);

    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "12px", width: "100%" }
    });

    // Mode Selector Buttons
    var modeRow = window.UI.h("div", {
      style: { display: "flex", gap: "8px", width: "100%" }
    });

    var blackBtn = window.UI.h("button", {
      className: "m3-button " + (!isWhite ? "filled" : "outlined"),
      style: { flex: "1", height: "44px" },
      attrs: { type: "button" }
    }, [window.UI.h("span", { text: "Чёрный список" })]);

    var whiteBtn = window.UI.h("button", {
      className: "m3-button " + (isWhite ? "filled" : "outlined"),
      style: { flex: "1", height: "44px" },
      attrs: { type: "button" }
    }, [window.UI.h("span", { text: "Белый список" })]);

    modeRow.appendChild(blackBtn);
    modeRow.appendChild(whiteBtn);
    content.appendChild(modeRow);

    // Add Input Row
    var addContainer = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "4px", width: "100%" }
    });

    var inputRow = window.UI.h("div", {
      style: { display: "flex", gap: "8px", alignItems: "center", width: "100%" }
    });

    var inputFieldContainer = window.UI.h("div", {
      className: "m3-field-container",
      style: { flex: "1", height: "46px" }
    });

    var inputEl = window.UI.h("input", {
      className: "m3-field-input",
      attrs: {
        type: "text",
        placeholder: "Например: telegram.exe или C:\\Program Files\\...",
        autocomplete: "off",
        spellcheck: "false"
      }
    });
    inputFieldContainer.appendChild(inputEl);

    var addBtn = window.UI.button({
      text: "Добавить",
      variant: "filled",
      icon: "Check",
      className: "compact"
    });
    addBtn.style.height = "46px";

    inputRow.appendChild(inputFieldContainer);
    inputRow.appendChild(addBtn);
    addContainer.appendChild(inputRow);

    var errorEl = window.UI.h("div", {
      className: "m3-field-error",
      style: { display: "none" }
    });
    addContainer.appendChild(errorEl);
    content.appendChild(addContainer);

    // Quick pickers: running processes and a file dialog
    var pickRow = window.UI.h("div", { style: { display: "flex", gap: "8px", width: "100%" } });
    var procBtn = window.UI.button({
      text: "Из запущенных", variant: "outlined", icon: "FilterList",
      onClick: function () { showProcessPicker(function (path) { addApp(path, false); }); }
    });
    var exeBtn = window.UI.button({
      text: "Выбрать exe…", variant: "outlined", icon: "ImportExport",
      onClick: function () {
        window.csqtt.pickExe().then(function (path) { if (path) addApp(path, false); })
          .catch(function (e) { setError(String(e && e.message || e)); });
      }
    });
    procBtn.style.flex = "1"; exeBtn.style.flex = "1";
    pickRow.appendChild(procBtn); pickRow.appendChild(exeBtn);
    content.appendChild(pickRow);

    // Counter Label
    var counterEl = window.UI.h("div", {
      className: "label-medium text-muted",
      style: { marginTop: "2px" }
    });
    content.appendChild(counterEl);

    // List Container
    var listWrap = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "6px",
        maxHeight: "260px",
        overflowY: "auto",
        paddingRight: "2px"
      }
    });
    content.appendChild(listWrap);

    function getActiveList() {
      return isWhite ? whiteList : blackList;
    }

    function setActiveList(newList) {
      if (isWhite) {
        whiteList = newList;
      } else {
        blackList = newList;
      }
    }

    function setError(msg) {
      if (msg) {
        errorEl.textContent = msg;
        errorEl.style.display = "block";
        inputFieldContainer.style.borderColor = "var(--md-sys-color-error)";
      } else {
        errorEl.textContent = "";
        errorEl.style.display = "none";
        inputFieldContainer.style.borderColor = "";
      }
    }

    function saveState() {
      var currentBlack = blackList.slice();
      var currentWhite = whiteList.slice();
      var currentMode = isWhite;
      return window.App.saveSettings(function (s) {
        var prof = s.profiles[s.active_profile || 0];
        if (!prof) return;
        prof.is_whitelist = currentMode;
        prof.excluded_apps = formatAppsList(currentBlack);
        prof.whitelist_apps = formatAppsList(currentWhite);
      });
    }

    function updateModeButtons() {
      if (!isWhite) {
        blackBtn.className = "m3-button filled";
        whiteBtn.className = "m3-button outlined";
      } else {
        blackBtn.className = "m3-button outlined";
        whiteBtn.className = "m3-button filled";
      }
    }

    function renderList() {
      listWrap.innerHTML = "";
      var activeList = getActiveList();
      counterEl.textContent = "Выбрано: " + activeList.length;

      if (activeList.length === 0) {
        var empty = window.UI.emptyState({
          title: "Ничего не добавлено",
          description: "Введите имя исполняемого файла (.exe) или полный путь выше",
          icon: "FilterList"
        });
        empty.style.padding = "24px 0";
        listWrap.appendChild(empty);
        return;
      }

      activeList.forEach(function (appItem, idx) {
        var row = window.UI.h("div", {
          style: {
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "8px 12px",
            borderRadius: "14px",
            backgroundColor: "var(--md-sys-color-surface-variant)",
            border: "1px solid var(--card-border)",
            gap: "8px"
          }
        });

        var tagIcon = window.Icons.el("Tag", 18);
        tagIcon.style.opacity = "0.7";
        tagIcon.style.flexShrink = "0";
        row.appendChild(tagIcon);

        var nameText = window.UI.h("span", {
          className: "body-medium",
          text: appItem,
          style: {
            flex: "1",
            wordBreak: "break-all",
            userSelect: "text",
            webkitUserSelect: "text"
          }
        });
        row.appendChild(nameText);

        var delBtn = window.UI.iconButton({
          icon: "Delete",
          size: 18,
          title: "Удалить",
          onClick: function () {
            var list = getActiveList().slice();
            list.splice(idx, 1);
            setActiveList(list);
            saveState();
            renderList();
          }
        });
        delBtn.style.width = "32px";
        delBtn.style.height = "32px";
        row.appendChild(delBtn);

        listWrap.appendChild(row);
      });
    }

    function handleAdd() {
      addApp(inputEl.value, true);
    }

    function addApp(raw, fromInput) {
      setError(null);
      var clean = cleanAppName(raw);

      if (!clean) {
        setError("Введите имя или путь к исполняемому файлу");
        inputEl.focus();
        return;
      }

      // Check duplicates (case-insensitive on Windows)
      var activeList = getActiveList();
      var cleanLower = clean.toLowerCase();
      var isDuplicate = activeList.some(function (item) {
        return item.toLowerCase() === cleanLower;
      });

      if (isDuplicate) {
        setError("Такое приложение уже есть в списке");
        inputEl.focus();
        return;
      }

      var next = activeList.slice();
      next.push(clean);
      setActiveList(next);
      if (fromInput) inputEl.value = "";
      setError(null);
      saveState();
      renderList();
      inputEl.focus();
    }

    blackBtn.addEventListener("click", function () {
      if (!isWhite) return;
      isWhite = false;
      updateModeButtons();
      setError(null);
      saveState();
      renderList();
    });

    whiteBtn.addEventListener("click", function () {
      if (isWhite) return;
      isWhite = true;
      updateModeButtons();
      setError(null);
      saveState();
      renderList();
    });

    addBtn.addEventListener("click", handleAdd);
    inputEl.addEventListener("keydown", function (e) {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAdd();
      }
    });

    var dlg = window.UI.dialog({
      title: "Список приложений",
      content: content,
      actions: [
        {
          text: "Готово",
          variant: "filled",
          onClick: function (d) { d.close(); }
        }
      ]
    });

    updateModeButtons();
    renderList();
    setTimeout(function () { inputEl.focus(); }, 120);
  }

  // --- Dialog: Domains Management ---

  function showDomainsDialog() {
    var p = window.App.profile() || {};
    var domainIsWhite = !!p.domain_is_whitelist;
    var templateEnabled = !domainIsWhite && !!p.template_domains_enabled;

    var blackRaw = (p.direct_domains_black || "").trim();
    if (!blackRaw && templateEnabled) {
      blackRaw = TEMPLATE_DOMAINS.join("\n");
    } else {
      blackRaw = parseDomainsFromText(blackRaw).join("\n");
    }
    var whiteRaw = parseDomainsFromText(p.direct_domains_white || "").join("\n");

    var currentTextBlack = blackRaw;
    var currentTextWhite = whiteRaw;

    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px", width: "100%" }
    });

    // Mode Selector Buttons
    var modeRow = window.UI.h("div", {
      style: { display: "flex", gap: "8px", width: "100%" }
    });

    var blackBtn = window.UI.h("button", {
      className: "m3-button " + (!domainIsWhite ? "filled" : "outlined"),
      style: { flex: "1", height: "44px" },
      attrs: { type: "button" }
    }, [window.UI.h("span", { text: "Чёрный список" })]);

    var whiteBtn = window.UI.h("button", {
      className: "m3-button " + (domainIsWhite ? "filled" : "outlined"),
      style: { flex: "1", height: "44px" },
      attrs: { type: "button" }
    }, [window.UI.h("span", { text: "Белый список" })]);

    modeRow.appendChild(blackBtn);
    modeRow.appendChild(whiteBtn);
    content.appendChild(modeRow);

    // Template Domains Row (visible only in blacklist mode)
    var templateWrap = window.UI.h("div", {
      style: { display: !domainIsWhite ? "block" : "none" }
    });

    var templateRow = window.UI.settingRow({
      title: "Готовый список RU доменов",
      description: "503 важных доменных имён",
      checked: templateEnabled,
      onCheckedChange: function (enabled) {
        templateEnabled = enabled;
        var currentList = parseDomainsFromText(textarea.value);
        if (enabled) {
          var seen = {};
          var merged = [];
          for (var j = 0; j < currentList.length; j++) {
            var d = currentList[j];
            if (!seen[d]) { seen[d] = true; merged.push(d); }
          }
          for (var k = 0; k < TEMPLATE_DOMAINS.length; k++) {
            var td = TEMPLATE_DOMAINS[k];
            if (!seen[td]) { seen[td] = true; merged.push(td); }
          }
          currentTextBlack = merged.join("\n");
          textarea.value = currentTextBlack;
        } else {
          var remaining = currentList.filter(function (d) {
            return !TEMPLATE_DOMAINS_SET[d];
          });
          currentTextBlack = remaining.join("\n");
          textarea.value = currentTextBlack;
        }
        updateCounter();
        saveCurrentState();
      }
    });
    templateWrap.appendChild(templateRow);
    content.appendChild(templateWrap);

    var div = window.UI.divider();
    content.appendChild(div);

    // Textarea
    var textarea = window.UI.h("textarea", {
      className: "m3-field-input",
      style: {
        width: "100%",
        minHeight: "160px",
        padding: "12px",
        borderRadius: "16px",
        border: "1px solid var(--md-sys-color-outline-variant)",
        backgroundColor: "transparent",
        color: "var(--md-sys-color-on-surface)",
        fontFamily: "var(--font-mono)",
        fontSize: "13px",
        lineHeight: "1.5",
        resize: "vertical",
        outline: "none",
        userSelect: "text",
        webkitUserSelect: "text"
      },
      attrs: {
        placeholder: "Введите домены списком, через запятую или пробел:\nyandex.ru, mail.ru\nkinopoisk.ru",
        spellcheck: "false"
      }
    });
    textarea.value = domainIsWhite ? currentTextWhite : currentTextBlack;
    content.appendChild(textarea);

    // Bottom Action Row
    var bottomRow = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        width: "100%",
        marginTop: "4px"
      }
    });

    var counterLabel = window.UI.h("span", {
      className: "label-medium text-muted"
    });
    bottomRow.appendChild(counterLabel);

    var rightBtns = window.UI.h("div", {
      style: { display: "flex", gap: "8px", alignItems: "center" }
    });

    var clearBtn = window.UI.button({
      text: "Очистить",
      variant: "outlined",
      className: "compact"
    });
    clearBtn.style.height = "36px";
    clearBtn.style.padding = "0 14px";

    var doneBtn = window.UI.button({
      text: "Готово",
      variant: "filled",
      className: "compact"
    });
    doneBtn.style.height = "36px";
    doneBtn.style.padding = "0 16px";

    rightBtns.appendChild(clearBtn);
    rightBtns.appendChild(doneBtn);
    bottomRow.appendChild(rightBtns);
    content.appendChild(bottomRow);

    function updateCounter() {
      var parsed = parseDomainsFromText(textarea.value);
      counterLabel.textContent = "Распознано: " + parsed.length;
      clearBtn.style.display = textarea.value.trim().length > 0 ? "inline-flex" : "none";
    }

    function updateModeDisplay() {
      if (!domainIsWhite) {
        blackBtn.className = "m3-button filled";
        whiteBtn.className = "m3-button outlined";
        templateWrap.style.display = "block";
      } else {
        blackBtn.className = "m3-button outlined";
        whiteBtn.className = "m3-button filled";
        templateWrap.style.display = "none";
      }
    }

    function saveCurrentState() {
      var currentMode = domainIsWhite;
      var curTemplate = currentMode ? false : templateEnabled;
      var parsedBlack = parseDomainsFromText(currentTextBlack);
      var parsedWhite = parseDomainsFromText(currentTextWhite);

      return window.App.saveSettings(function (s) {
        var prof = s.profiles[s.active_profile || 0];
        if (!prof) return;
        prof.domain_is_whitelist = currentMode;
        prof.template_domains_enabled = curTemplate;
        prof.direct_domains_black = parsedBlack.join(",");
        prof.direct_domains_white = parsedWhite.join(",");
      });
    }

    textarea.addEventListener("input", function () {
      if (domainIsWhite) {
        currentTextWhite = textarea.value;
      } else {
        currentTextBlack = textarea.value;
      }
      updateCounter();
    });

    blackBtn.addEventListener("click", function () {
      if (!domainIsWhite) return;
      currentTextWhite = textarea.value;
      domainIsWhite = false;
      textarea.value = currentTextBlack;
      updateModeDisplay();
      updateCounter();
      saveCurrentState();
    });

    whiteBtn.addEventListener("click", function () {
      if (domainIsWhite) return;
      currentTextBlack = textarea.value;
      domainIsWhite = true;
      textarea.value = currentTextWhite;
      updateModeDisplay();
      updateCounter();
      saveCurrentState();
    });

    clearBtn.addEventListener("click", function () {
      if (domainIsWhite) {
        currentTextWhite = "";
      } else {
        currentTextBlack = "";
        templateEnabled = false;
        templateRow.setChecked(false);
      }
      textarea.value = "";
      updateCounter();
      saveCurrentState();
    });

    var dlg = window.UI.dialog({
      title: "Маршрутизация доменов",
      content: content,
      onClose: function () {
        saveCurrentState();
      }
    });

    doneBtn.addEventListener("click", function () {
      saveCurrentState().then(function () {
        dlg.close();
      });
    });

    updateModeDisplay();
    updateCounter();
  }

  // --- Dialog: IP Routing Management ---

  function showIpsDialog() {
    var p = window.App.profile() || {};
    var ipIsWhite = !!p.ip_is_whitelist;

    var blackRaw = parseIpsFromText(p.direct_ips_black || "").join("\n");
    var whiteRaw = parseIpsFromText(p.direct_ips_white || "").join("\n");

    var currentTextBlack = blackRaw;
    var currentTextWhite = whiteRaw;

    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px", width: "100%" }
    });

    // Mode Selector Buttons
    var modeRow = window.UI.h("div", {
      style: { display: "flex", gap: "8px", width: "100%" }
    });

    var blackBtn = window.UI.h("button", {
      className: "m3-button " + (!ipIsWhite ? "filled" : "outlined"),
      style: { flex: "1", height: "44px" },
      attrs: { type: "button" }
    }, [window.UI.h("span", { text: "Чёрный список" })]);

    var whiteBtn = window.UI.h("button", {
      className: "m3-button " + (ipIsWhite ? "filled" : "outlined"),
      style: { flex: "1", height: "44px" },
      attrs: { type: "button" }
    }, [window.UI.h("span", { text: "Белый список" })]);

    modeRow.appendChild(blackBtn);
    modeRow.appendChild(whiteBtn);
    content.appendChild(modeRow);

    var div = window.UI.divider();
    content.appendChild(div);

    // Textarea
    var textarea = window.UI.h("textarea", {
      className: "m3-field-input",
      style: {
        width: "100%",
        minHeight: "160px",
        padding: "12px",
        borderRadius: "16px",
        border: "1px solid var(--md-sys-color-outline-variant)",
        backgroundColor: "transparent",
        color: "var(--md-sys-color-on-surface)",
        fontFamily: "var(--font-mono)",
        fontSize: "13px",
        lineHeight: "1.5",
        resize: "vertical",
        outline: "none",
        userSelect: "text",
        webkitUserSelect: "text"
      },
      attrs: {
        placeholder: "1.2.3.4/32\n1.2.3.4\n1.2.3.4-1.2.3.255",
        spellcheck: "false"
      }
    });
    textarea.value = ipIsWhite ? currentTextWhite : currentTextBlack;
    content.appendChild(textarea);

    // Bottom Action Row
    var bottomRow = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        width: "100%",
        marginTop: "4px"
      }
    });

    var counterLabel = window.UI.h("span", {
      className: "label-medium text-muted"
    });
    bottomRow.appendChild(counterLabel);

    var rightBtns = window.UI.h("div", {
      style: { display: "flex", gap: "8px", alignItems: "center" }
    });

    var clearBtn = window.UI.button({
      text: "Очистить",
      variant: "outlined",
      className: "compact"
    });
    clearBtn.style.height = "36px";
    clearBtn.style.padding = "0 14px";

    var doneBtn = window.UI.button({
      text: "Готово",
      variant: "filled",
      className: "compact"
    });
    doneBtn.style.height = "36px";
    doneBtn.style.padding = "0 16px";

    rightBtns.appendChild(clearBtn);
    rightBtns.appendChild(doneBtn);
    bottomRow.appendChild(rightBtns);
    content.appendChild(bottomRow);

    function updateCounter() {
      var parsed = parseIpsFromText(textarea.value);
      counterLabel.textContent = "Распознано: " + parsed.length;
      clearBtn.style.display = textarea.value.trim().length > 0 ? "inline-flex" : "none";
    }

    function updateModeDisplay() {
      if (!ipIsWhite) {
        blackBtn.className = "m3-button filled";
        whiteBtn.className = "m3-button outlined";
      } else {
        blackBtn.className = "m3-button outlined";
        whiteBtn.className = "m3-button filled";
      }
    }

    function saveCurrentState() {
      var currentMode = ipIsWhite;
      var parsedBlack = parseIpsFromText(currentTextBlack);
      var parsedWhite = parseIpsFromText(currentTextWhite);

      return window.App.saveSettings(function (s) {
        var prof = s.profiles[s.active_profile || 0];
        if (!prof) return;
        prof.ip_is_whitelist = currentMode;
        prof.ip_routing_enabled = true;
        prof.direct_ips_black = parsedBlack.join(",");
        prof.direct_ips_white = parsedWhite.join(",");
      });
    }

    textarea.addEventListener("input", function () {
      if (ipIsWhite) {
        currentTextWhite = textarea.value;
      } else {
        currentTextBlack = textarea.value;
      }
      updateCounter();
    });

    blackBtn.addEventListener("click", function () {
      if (!ipIsWhite) return;
      currentTextWhite = textarea.value;
      ipIsWhite = false;
      textarea.value = currentTextBlack;
      updateModeDisplay();
      updateCounter();
      saveCurrentState();
    });

    whiteBtn.addEventListener("click", function () {
      if (ipIsWhite) return;
      currentTextBlack = textarea.value;
      ipIsWhite = true;
      textarea.value = currentTextWhite;
      updateModeDisplay();
      updateCounter();
      saveCurrentState();
    });

    clearBtn.addEventListener("click", function () {
      if (ipIsWhite) {
        currentTextWhite = "";
      } else {
        currentTextBlack = "";
      }
      textarea.value = "";
      updateCounter();
      saveCurrentState();
    });

    var dlg = window.UI.dialog({
      title: "Маршрутизация адресов",
      content: content,
      onClose: function () {
        saveCurrentState();
      }
    });

    doneBtn.addEventListener("click", function () {
      saveCurrentState().then(function () {
        dlg.close();
      });
    });

    updateModeDisplay();
    updateCounter();
  }

  // --- Dialog: Rules Transfer (Import / Export) ---

  function showRulesTransferDialog() {
    var transferApps = true;
    var transferDomains = true;
    var transferIps = true;

    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "12px", width: "100%" }
    });

    function createCheckboxRow(label, initialChecked, onChange) {
      var checked = initialChecked;
      var row = window.UI.h("div", {
        className: "section-card",
        style: {
          padding: "12px 16px",
          display: "flex",
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "space-between",
          cursor: "pointer",
          borderRadius: "16px",
          backgroundColor: "var(--md-sys-color-surface-variant)"
        }
      });

      var labelEl = window.UI.h("span", {
        className: "body-large",
        text: label,
        style: { fontWeight: "500" }
      });
      row.appendChild(labelEl);

      var checkEl = window.UI.h("div", {
        style: {
          width: "22px",
          height: "22px",
          borderRadius: "6px",
          border: "2px solid var(--md-sys-color-primary)",
          backgroundColor: checked ? "var(--md-sys-color-primary)" : "transparent",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--md-sys-color-on-primary)",
          transition: "all 140ms ease"
        }
      });
      if (checked) {
        checkEl.appendChild(window.Icons.el("Check", 16));
      }
      row.appendChild(checkEl);

      row.addEventListener("click", function () {
        checked = !checked;
        checkEl.innerHTML = "";
        if (checked) {
          checkEl.style.backgroundColor = "var(--md-sys-color-primary)";
          checkEl.appendChild(window.Icons.el("Check", 16));
        } else {
          checkEl.style.backgroundColor = "transparent";
        }
        onChange(checked);
      });

      return row;
    }

    content.appendChild(createCheckboxRow("Приложения", transferApps, function (c) {
      transferApps = c;
    }));
    content.appendChild(createCheckboxRow("Домены", transferDomains, function (c) {
      transferDomains = c;
    }));
    content.appendChild(createCheckboxRow("IP адреса", transferIps, function (c) {
      transferIps = c;
    }));

    var buttonsRow = window.UI.h("div", {
      style: { display: "flex", gap: "10px", width: "100%", marginTop: "6px" }
    });

    var exportBtn = window.UI.button({
      text: "Экспорт",
      variant: "filled",
      icon: "ArrowDownward"
    });
    exportBtn.style.flex = "1";

    var importBtn = window.UI.button({
      text: "Импорт",
      variant: "filled",
      icon: "Login"
    });
    importBtn.style.flex = "1";

    buttonsRow.appendChild(exportBtn);
    buttonsRow.appendChild(importBtn);
    content.appendChild(buttonsRow);

    var dlg = window.UI.dialog({
      title: "Импорт и экспорт правил",
      content: content
    });

    exportBtn.addEventListener("click", function () {
      if (!transferApps && !transferDomains && !transferIps) {
        window.UI.toast("Выберите хотя бы один пункт");
        return;
      }

      var p = window.App.profile() || {};
      var root = { version: 2 };

      if (transferApps) {
        var isWhiteApps = !!p.is_whitelist;
        var blackApps = parseAppsList(p.excluded_apps).sort();
        var whiteApps = parseAppsList(p.whitelist_apps).sort();
        root.apps = {
          is_whitelist: isWhiteApps,
          active_mode: isWhiteApps ? "whitelist" : "blacklist",
          blacklist: blackApps,
          whitelist: whiteApps
        };
      }

      if (transferDomains) {
        var isWhiteDomains = !!p.domain_is_whitelist;
        var templateDom = !isWhiteDomains && !!p.template_domains_enabled;
        var blackDom = parseDomainsFromText(p.direct_domains_black || "");
        var whiteDom = parseDomainsFromText(p.direct_domains_white || "");

        var customBlack = templateDom
          ? blackDom.filter(function (d) { return !TEMPLATE_DOMAINS_SET[d]; })
          : blackDom;

        root.domains = {
          is_whitelist: isWhiteDomains,
          active_mode: isWhiteDomains ? "whitelist" : "blacklist",
          template_domains: templateDom,
          blacklist: customBlack,
          whitelist: whiteDom,
          direct_domains: isWhiteDomains ? whiteDom : customBlack
        };
      }

      if (transferIps) {
        var isWhiteIps = !!p.ip_is_whitelist;
        var blackIps = parseIpsFromText(p.direct_ips_black || "");
        var whiteIps = parseIpsFromText(p.direct_ips_white || "");

        root.ips = {
          is_whitelist: isWhiteIps,
          active_mode: isWhiteIps ? "whitelist" : "blacklist",
          blacklist: blackIps,
          whitelist: whiteIps,
          direct_ips: isWhiteIps ? whiteIps : blackIps
        };
      }

      var jsonStr = JSON.stringify(root, null, 2);
      copyToClipboard(jsonStr).then(function () {
        window.UI.toast("Правила экспортированы в буфер обмена");
      }).catch(function () {
        // Fallback: show dialog with text to copy manually
        window.UI.dialog({
          title: "Экспорт правил",
          content: window.UI.h("textarea", {
            style: { width: "100%", height: "200px", fontFamily: "var(--font-mono)", fontSize: "12px" },
            attrs: { readonly: "readonly" },
            text: jsonStr
          }),
          actions: [{ text: "Закрыть", variant: "text", onClick: function (d) { d.close(); } }]
        });
      });
    });

    function applyImportedText(clipText) {
      var trimmed = (clipText || "").trim();
      if (!trimmed) {
        window.UI.toast("Буфер обмена пуст");
        return;
      }

      var importedAny = false;
      var rootJson = null;
      try {
        rootJson = JSON.parse(trimmed);
      } catch (_) {
        rootJson = null;
      }

      if (rootJson && typeof rootJson === "object") {
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (!prof) return;

          if (transferApps && rootJson.apps) {
            var appsObj = rootJson.apps;
            var isWhite = appsObj.active_mode === "whitelist" || !!appsObj.is_whitelist;
            prof.is_whitelist = isWhite;

            var newBlack = [];
            if (Array.isArray(appsObj.blacklist)) {
              appsObj.blacklist.forEach(function (app) {
                var c = cleanAppName(app);
                if (c && newBlack.indexOf(c) === -1) newBlack.push(c);
              });
            }
            var newWhite = [];
            if (Array.isArray(appsObj.whitelist)) {
              appsObj.whitelist.forEach(function (app) {
                var c = cleanAppName(app);
                if (c && newWhite.indexOf(c) === -1) newWhite.push(c);
              });
            }
            prof.excluded_apps = newBlack.join(",");
            prof.whitelist_apps = newWhite.join(",");
            importedAny = true;
          }

          if (transferDomains && rootJson.domains) {
            var domObj = rootJson.domains;
            var isWhiteDom = domObj.active_mode === "whitelist" || !!domObj.is_whitelist;
            var isTemplate = !!domObj.template_domains;

            var impBlack = [];
            if (Array.isArray(domObj.blacklist)) {
              domObj.blacklist.forEach(function (d) {
                var cd = cleanDomain(d);
                if (isValidDomain(cd) && impBlack.indexOf(cd) === -1) impBlack.push(cd);
              });
            }
            var impWhite = [];
            if (Array.isArray(domObj.whitelist)) {
              domObj.whitelist.forEach(function (d) {
                var cd = cleanDomain(d);
                if (isValidDomain(cd) && impWhite.indexOf(cd) === -1) impWhite.push(cd);
              });
            }
            if (Array.isArray(domObj.direct_domains) && impBlack.length === 0 && impWhite.length === 0) {
              domObj.direct_domains.forEach(function (d) {
                var cd = cleanDomain(d);
                if (isValidDomain(cd)) {
                  if (isWhiteDom && impWhite.indexOf(cd) === -1) impWhite.push(cd);
                  else if (!isWhiteDom && impBlack.indexOf(cd) === -1) impBlack.push(cd);
                }
              });
            }

            prof.domain_is_whitelist = isWhiteDom;
            prof.template_domains_enabled = isTemplate;
            if (impBlack.length > 0 || !isWhiteDom) prof.direct_domains_black = impBlack.join(",");
            if (impWhite.length > 0 || isWhiteDom) prof.direct_domains_white = impWhite.join(",");
            importedAny = true;
          }

          if (transferIps && rootJson.ips) {
            var ipsObj = rootJson.ips;
            var isWhiteIps = ipsObj.active_mode === "whitelist" || !!ipsObj.is_whitelist;

            var impBlackIps = [];
            if (Array.isArray(ipsObj.blacklist)) {
              ipsObj.blacklist.forEach(function (r) {
                var cr = cleanIpRule(r);
                if (isValidIpRule(cr) && impBlackIps.indexOf(cr) === -1) impBlackIps.push(cr);
              });
            }
            var impWhiteIps = [];
            if (Array.isArray(ipsObj.whitelist)) {
              ipsObj.whitelist.forEach(function (r) {
                var cr = cleanIpRule(r);
                if (isValidIpRule(cr) && impWhiteIps.indexOf(cr) === -1) impWhiteIps.push(cr);
              });
            }
            if (Array.isArray(ipsObj.direct_ips) && impBlackIps.length === 0 && impWhiteIps.length === 0) {
              ipsObj.direct_ips.forEach(function (r) {
                var cr = cleanIpRule(r);
                if (isValidIpRule(cr)) {
                  if (isWhiteIps && impWhiteIps.indexOf(cr) === -1) impWhiteIps.push(cr);
                  else if (!isWhiteIps && impBlackIps.indexOf(cr) === -1) impBlackIps.push(cr);
                }
              });
            }

            prof.ip_is_whitelist = isWhiteIps;
            prof.ip_routing_enabled = true;
            if (impBlackIps.length > 0 || !isWhiteIps) prof.direct_ips_black = impBlackIps.join(",");
            if (impWhiteIps.length > 0 || isWhiteIps) prof.direct_ips_white = impWhiteIps.join(",");
            importedAny = true;
          }
        }).then(function () {
          if (importedAny) {
            window.UI.toast("Правила успешно импортированы");
            dlg.close();
          } else {
            window.UI.toast("В буфере обмена нет подходящих данных");
          }
        });
      } else {
        // Plain text parsing fallback
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (!prof) return;

          if (transferApps) {
            var lines = trimmed.split(/\r?\n/);
            var currentSection = null;
            var blackApps = parseAppsList(prof.excluded_apps);
            var whiteApps = parseAppsList(prof.whitelist_apps);
            var textHeaderFound = false;

            for (var l = 0; l < lines.length; l++) {
              var line = lines[l].trim();
              if (!line) continue;
              var lower = line.toLowerCase();
              if (lower.indexOf("чёрный-список:") === 0 || lower.indexOf("черный-список:") === 0 ||
                  lower.indexOf("чёрный список:") === 0 || lower.indexOf("черный список:") === 0 ||
                  lower.indexOf("blacklist:") === 0 || lower.indexOf("black:") === 0) {
                textHeaderFound = true;
                currentSection = "black";
                var rem = cleanAppName(line.slice(line.indexOf(":") + 1));
                if (rem && blackApps.indexOf(rem) === -1) blackApps.push(rem);
              } else if (lower.indexOf("белый-список:") === 0 || lower.indexOf("белый список:") === 0 ||
                         lower.indexOf("whitelist:") === 0 || lower.indexOf("white:") === 0) {
                textHeaderFound = true;
                currentSection = "white";
                var remW = cleanAppName(line.slice(line.indexOf(":") + 1));
                if (remW && whiteApps.indexOf(remW) === -1) whiteApps.push(remW);
              } else if (currentSection === "black") {
                var c = cleanAppName(line);
                if (c && blackApps.indexOf(c) === -1) blackApps.push(c);
              } else if (currentSection === "white") {
                var cw = cleanAppName(line);
                if (cw && whiteApps.indexOf(cw) === -1) whiteApps.push(cw);
              }
            }

            if (textHeaderFound) {
              prof.excluded_apps = blackApps.join(",");
              prof.whitelist_apps = whiteApps.join(",");
              importedAny = true;
            }
          }

          if (transferDomains) {
            var importedDomains = parseDomainsFromText(trimmed);
            if (importedDomains.length > 0) {
              var curDom = prof.domain_is_whitelist ? prof.direct_domains_white : prof.direct_domains_black;
              var existingList = parseDomainsFromText(curDom);
              var seenDom = {};
              var combinedDom = [];
              existingList.forEach(function (d) { seenDom[d] = true; combinedDom.push(d); });
              importedDomains.forEach(function (d) {
                if (!seenDom[d]) { seenDom[d] = true; combinedDom.push(d); }
              });
              var formattedDom = combinedDom.slice(0, 1024).join(",");
              if (prof.domain_is_whitelist) {
                prof.direct_domains_white = formattedDom;
              } else {
                prof.direct_domains_black = formattedDom;
              }
              importedAny = true;
            }
          }

          if (transferIps) {
            var importedIps = parseIpsFromText(trimmed);
            if (importedIps.length > 0) {
              var curIps = prof.ip_is_whitelist ? prof.direct_ips_white : prof.direct_ips_black;
              var existingIps = parseIpsFromText(curIps);
              var seenIps = {};
              var combinedIps = [];
              existingIps.forEach(function (r) { seenIps[r] = true; combinedIps.push(r); });
              importedIps.forEach(function (r) {
                if (!seenIps[r]) { seenIps[r] = true; combinedIps.push(r); }
              });
              var formattedIps = combinedIps.slice(0, 1024).join(",");
              prof.ip_routing_enabled = true;
              if (prof.ip_is_whitelist) {
                prof.direct_ips_white = formattedIps;
              } else {
                prof.direct_ips_black = formattedIps;
              }
              importedAny = true;
            }
          }
        }).then(function () {
          if (importedAny) {
            window.UI.toast("Правила успешно импортированы");
            dlg.close();
          } else {
            window.UI.toast("В буфере обмена нет подходящих данных");
          }
        });
      }
    }

    importBtn.addEventListener("click", function () {
      if (!transferApps && !transferDomains && !transferIps) {
        window.UI.toast("Выберите хотя бы один пункт");
        return;
      }

      readFromClipboard().then(function (text) {
        applyImportedText(text);
      }).catch(function () {
        // Fallback dialog with manual paste textarea
        var pasteTa = window.UI.h("textarea", {
          style: { width: "100%", height: "180px", fontFamily: "var(--font-mono)", fontSize: "12px", padding: "8px" },
          attrs: { placeholder: "Вставьте правила (JSON или текст) сюда..." }
        });
        window.UI.dialog({
          title: "Импорт правил из текста",
          content: pasteTa,
          actions: [
            { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
            {
              text: "Импортировать",
              variant: "filled",
              onClick: function (d) {
                var val = pasteTa.value;
                d.close();
                applyImportedText(val);
              }
            }
          ]
        });
      });
    });
  }

  // --- Main Screen Renderer ---

  function renderExceptionsScreen(container) {
    container.innerHTML = "";
    container.className = "screen-container";

    // Server Routing Active Banner Container
    var serverBannerCard = window.UI.h("div", {
      style: {
        display: "none",
        width: "100%",
        padding: "32px 16px",
        alignItems: "center",
        justifyContent: "center",
        textAlign: "center"
      }
    });
    var serverBannerText = window.UI.h("span", {
      className: "title-medium",
      text: "Маршрутизация синхронизирована с сервером",
      style: { color: "#2563EB", fontWeight: "600" }
    });
    serverBannerCard.appendChild(serverBannerText);
    container.appendChild(serverBannerCard);

    // Cards Container
    var cardsWrap = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "var(--spacing-md)", width: "100%" }
    });
    container.appendChild(cardsWrap);

    // 1. App Routing Card
    var appsCard = window.UI.sectionCard({ gap: "12px" });
    var appsHeader = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "4px" }
    });
    var appsTitle = window.UI.h("span", {
      className: "title-medium",
      text: "Маршрутизация приложений",
      style: { fontWeight: "700" }
    });
    var appsSubtitle = window.UI.h("span", {
      className: "body-small text-muted"
    });
    appsHeader.appendChild(appsTitle);
    appsHeader.appendChild(appsSubtitle);
    appsCard.appendChild(appsHeader);

    var appsBtn = window.UI.button({
      text: "Список приложений",
      variant: "outlined",
      icon: "FilterList",
      fullWidth: true,
      onClick: showAppsDialog
    });
    appsCard.appendChild(appsBtn);
    cardsWrap.appendChild(appsCard);

    // 2. Domain Routing Card
    var domainsCard = window.UI.sectionCard({ gap: "12px" });
    var domainsHeader = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "4px" }
    });
    var domainsTitle = window.UI.h("span", {
      className: "title-medium",
      text: "Маршрутизация доменов",
      style: { fontWeight: "700" }
    });
    var domainsSubtitle = window.UI.h("span", {
      className: "body-small text-muted"
    });
    domainsHeader.appendChild(domainsTitle);
    domainsHeader.appendChild(domainsSubtitle);
    domainsCard.appendChild(domainsHeader);

    var domainsBtn = window.UI.button({
      text: "Список доменов",
      variant: "outlined",
      icon: "Dns",
      fullWidth: true,
      onClick: showDomainsDialog
    });
    domainsCard.appendChild(domainsBtn);
    cardsWrap.appendChild(domainsCard);

    // 3. IP Routing Card
    var ipsCard = window.UI.sectionCard({ gap: "12px" });
    var ipsHeader = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "4px" }
    });
    var ipsTitle = window.UI.h("span", {
      className: "title-medium",
      text: "Маршрутизация адресов",
      style: { fontWeight: "700" }
    });
    var ipsSubtitle = window.UI.h("span", {
      className: "body-small text-muted"
    });
    ipsHeader.appendChild(ipsTitle);
    ipsHeader.appendChild(ipsSubtitle);
    ipsCard.appendChild(ipsHeader);

    var ipsBtn = window.UI.button({
      text: "Список адресов",
      variant: "outlined",
      icon: "Dns",
      fullWidth: true,
      onClick: showIpsDialog
    });
    ipsCard.appendChild(ipsBtn);
    cardsWrap.appendChild(ipsCard);

    // 4. Rules Transfer Card
    var transferCard = window.UI.sectionCard({ gap: "10px" });
    var transferHeader = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "2px" }
    });
    var transferTitle = window.UI.h("span", {
      className: "title-medium",
      text: "Импорт и экспорт правил",
      style: { fontWeight: "700" }
    });
    var transferSubtitle = window.UI.h("span", {
      className: "body-small text-muted",
      text: "Резервное копирование и перенос настроек"
    });
    transferHeader.appendChild(transferTitle);
    transferHeader.appendChild(transferSubtitle);
    transferCard.appendChild(transferHeader);

    var transferBtn = window.UI.button({
      text: "Импорт и экспорт правил",
      variant: "outlined",
      icon: "ImportExport",
      fullWidth: true,
      onClick: showRulesTransferDialog
    });
    transferCard.appendChild(transferBtn);
    cardsWrap.appendChild(transferCard);

    function updateUI(appState) {
      var isServerRouting = !!(appState && appState.status && appState.status.server_routing_active);
      if (isServerRouting) {
        serverBannerCard.style.display = "flex";
        cardsWrap.style.display = "none";
        return;
      }
      serverBannerCard.style.display = "none";
      cardsWrap.style.display = "flex";

      var p = window.App.profile() || {};

      // 1. Apps Subtitle
      var isWhiteApps = !!p.is_whitelist;
      var blackApps = parseAppsList(p.excluded_apps);
      var whiteApps = parseAppsList(p.whitelist_apps);
      if (isWhiteApps) {
        appsSubtitle.textContent = "Белый список: " + whiteApps.length + " в туннеле";
      } else {
        appsSubtitle.textContent = "Чёрный список: " + blackApps.length + " исключено";
      }

      // 2. Domains Subtitle
      var isWhiteDom = !!p.domain_is_whitelist;
      var templateEnabled = !isWhiteDom && !!p.template_domains_enabled;
      var blackDom = parseDomainsFromText(p.direct_domains_black || "");
      var whiteDom = parseDomainsFromText(p.direct_domains_white || "");

      if (isWhiteDom) {
        domainsSubtitle.textContent = "Белый список: " + whiteDom.length + " в туннеле";
      } else if (blackDom.length === 0) {
        domainsSubtitle.textContent = "Чёрный список: 0 доменов исключено";
      } else if (templateEnabled) {
        var customCount = 0;
        for (var dIdx = 0; dIdx < blackDom.length; dIdx++) {
          if (!TEMPLATE_DOMAINS_SET[blackDom[dIdx]]) customCount++;
        }
        if (customCount > 0) {
          domainsSubtitle.textContent = "Исключено: " + TEMPLATE_DOMAINS.length + " готовых + " + customCount + " своих";
        } else {
          domainsSubtitle.textContent = "Исключено: " + blackDom.length + " (Готовый список RU)";
        }
      } else {
        domainsSubtitle.textContent = "Чёрный список: " + blackDom.length + " исключено";
      }

      // 3. IPs Subtitle
      var isWhiteIps = !!p.ip_is_whitelist;
      var blackIps = parseIpsFromText(p.direct_ips_black || "");
      var whiteIps = parseIpsFromText(p.direct_ips_white || "");

      if (isWhiteIps) {
        ipsSubtitle.textContent = "Белый список: " + whiteIps.length + " в туннеле";
      } else {
        ipsSubtitle.textContent = "Чёрный список: " + blackIps.length + " исключено";
      }
    }

    // Subscribe to state updates
    window.App.onState(updateUI);
  }

  // Register Exceptions Screen (order 4)
  window.App.registerScreen({
    id: "exceptions",
    title: "Маршрут",
    icon: "FilterList",
    order: 4,
    render: renderExceptionsScreen
  });

  // Expose Public API
  window.Exceptions = {
    cleanDomain: cleanDomain,
    isValidDomain: isValidDomain,
    parseDomainsFromText: parseDomainsFromText,
    isValidIpv4: isValidIpv4,
    isValidIpRule: isValidIpRule,
    cleanIpRule: cleanIpRule,
    parseIpsFromText: parseIpsFromText,
    cleanAppName: cleanAppName,
    parseAppsList: parseAppsList,
    formatAppsList: formatAppsList,
    templateDomains: TEMPLATE_DOMAINS,
    showAppsDialog: showAppsDialog,
    showDomainsDialog: showDomainsDialog,
    showIpsDialog: showIpsDialog,
    showRulesTransferDialog: showRulesTransferDialog
  };
})();
