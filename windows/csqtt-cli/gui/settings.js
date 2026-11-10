/**
 * Tunnel and Settings Screen for csqtt-win (Material 3)
 *
 * Implements the Tunnel / Settings screen mirroring Android SettingsTab.kt
 * and FloatingToolbar.kt.
 *
 * Registered as Screen ID "tunnel" (order 1) in window.App navigation.
 *
 * Features:
 * - Profile Management (Active profile 0..3, rename profile, reset profile, switch profile)
 * - Usage Role selection (Owner / Participant csqtt_link_mode)
 * - Participant Mode: csqtt:// link input, validation, token/hashes indicators
 * - Owner Mode: Server IP / domain input, manual PEER port input
 * - VK Hashes Management (HashesDialog: up to 6 hashes, paste, format validation, duplicate check, check active)
 * - VK Hash Modes (Manual, Auto API, Auto ВК) with mode indicators and info dialogs
 * - VK Work Modes (Captcha/legacy, Auto/vkcalls, Auto ВК/auto_js) with mode indicators and info dialogs
 * - SRTP Modes (WRAP-SRTP, DTLS-SRTP, Auto)
 * - Transport Selection (UDP, TCP) with experimental TCP risk acknowledgment dialog
 * - Turn Workers Slider (5..100 workers, 2 Mbps/worker bandwidth display, WorkersInfoDialog)
 * - Captcha settings (Auto captcha toggle, WBV/RJS method chips, Manual/Auto solve chips)
 * - SNI configuration
 * - Authorization (SecretsDialog for connection password encrypted storage)
 * - Turn Subnets Dialog (subnets list, pool sizes, ping latency testing, IP limits)
 * - Theme mode selection (System, Light, Dark) bound to dataset.theme
 * - Wi-Fi auto-pause toggle with info dialog
 * - Fingerprint selection (Chrome, Safari, Firefox, Edge, Opera)
 */

(function () {
  "use strict";

  // Storage Secret Keys (matching settings.rs & SecureStringStore)
  var SECRET_CONNECTION_PASSWORD = "connection_password_encrypted";
  var SECRET_VK_TOKEN = "vk_access_token_encrypted";

  var DEFAULT_SERVER_PEER_PORT = 46000;
  var MIN_WORKERS = 10;
  var MAX_WORKERS = 100;
  var WORKERS_STEP = 10;
  var WORKERS_PER_GROUP = 10;

  function roundWorkersToStep(val) {
    var num = Number(val);
    if (isNaN(num)) num = MIN_WORKERS;
    var rounded = Math.round(num / WORKERS_STEP) * WORKERS_STEP;
    if (rounded < MIN_WORKERS) rounded = MIN_WORKERS;
    if (rounded > MAX_WORKERS) rounded = MAX_WORKERS;
    return rounded;
  }

  // Known TURN Subnets mirroring TurnSubnetRegistry.kt / turn_subnet_registry.rs
  var TURN_SUBNETS = [
    {
      id: "95_163_34_0_24",
      title: "95.163.34.0/24",
      cidr: "95.163.34.0/24",
      count: 64,
      ips: [
        "95.163.34.48", "95.163.34.128", "95.163.34.129", "95.163.34.130", "95.163.34.131",
        "95.163.34.132", "95.163.34.133", "95.163.34.134", "95.163.34.135", "95.163.34.136",
        "95.163.34.137", "95.163.34.138", "95.163.34.139", "95.163.34.140", "95.163.34.141",
        "95.163.34.142", "95.163.34.143", "95.163.34.144", "95.163.34.145", "95.163.34.146",
        "95.163.34.147", "95.163.34.148", "95.163.34.149", "95.163.34.150", "95.163.34.151",
        "95.163.34.152", "95.163.34.153", "95.163.34.154", "95.163.34.155", "95.163.34.156",
        "95.163.34.157", "95.163.34.158", "95.163.34.159", "95.163.34.160", "95.163.34.161",
        "95.163.34.162", "95.163.34.163", "95.163.34.164", "95.163.34.165", "95.163.34.166",
        "95.163.34.167", "95.163.34.168", "95.163.34.169", "95.163.34.170", "95.163.34.171",
        "95.163.34.172", "95.163.34.173", "95.163.34.174", "95.163.34.175", "95.163.34.176",
        "95.163.34.177", "95.163.34.178", "95.163.34.179", "95.163.34.180", "95.163.34.181",
        "95.163.34.182", "95.163.34.183", "95.163.34.184", "95.163.34.185", "95.163.34.186",
        "95.163.34.187", "95.163.34.188", "95.163.34.189", "95.163.34.190"
      ]
    },
    {
      id: "91_231_135_0_24",
      title: "91.231.135.0/24",
      cidr: "91.231.135.0/24",
      count: 64,
      ips: [
        "91.231.135.85", "91.231.135.86", "91.231.135.87", "91.231.135.88", "91.231.135.89",
        "91.231.135.90", "91.231.135.91", "91.231.135.92", "91.231.135.93", "91.231.135.94",
        "91.231.135.95", "91.231.135.128", "91.231.135.129", "91.231.135.130", "91.231.135.131",
        "91.231.135.132", "91.231.135.133", "91.231.135.134", "91.231.135.135", "91.231.135.136",
        "91.231.135.137", "91.231.135.138", "91.231.135.139", "91.231.135.140", "91.231.135.141",
        "91.231.135.142", "91.231.135.143", "91.231.135.144", "91.231.135.145", "91.231.135.146",
        "91.231.135.147", "91.231.135.148", "91.231.135.149", "91.231.135.150", "91.231.135.151",
        "91.231.135.152", "91.231.135.153", "91.231.135.154", "91.231.135.155", "91.231.135.167",
        "91.231.135.168", "91.231.135.169", "91.231.135.170", "91.231.135.171", "91.231.135.172",
        "91.231.135.173", "91.231.135.174", "91.231.135.175", "91.231.135.176", "91.231.135.177",
        "91.231.135.178", "91.231.135.179", "91.231.135.180", "91.231.135.181", "91.231.135.182",
        "91.231.135.183", "91.231.135.184", "91.231.135.185", "91.231.135.186", "91.231.135.187",
        "91.231.135.188", "91.231.135.189", "91.231.135.190", "91.231.135.191"
      ]
    },
    {
      id: "90_156_232_0_21",
      title: "90.156.232.0/21",
      cidr: "90.156.232.0/21",
      count: 51,
      ips: [
        "90.156.234.253", "90.156.234.254", "90.156.234.255", "90.156.236.64", "90.156.236.65",
        "90.156.236.66", "90.156.236.67", "90.156.236.68", "90.156.236.69", "90.156.236.70",
        "90.156.236.71", "90.156.236.72", "90.156.236.73", "90.156.236.74", "90.156.236.75",
        "90.156.236.76", "90.156.236.77", "90.156.236.78", "90.156.236.79", "90.156.236.80",
        "90.156.236.81", "90.156.236.82", "90.156.236.83", "90.156.236.84", "90.156.236.85",
        "90.156.236.86", "90.156.236.87", "90.156.236.88", "90.156.236.89", "90.156.236.90",
        "90.156.236.91", "90.156.236.92", "90.156.236.93", "90.156.236.94", "90.156.236.95",
        "90.156.236.96", "90.156.236.97", "90.156.236.98", "90.156.236.99", "90.156.236.100",
        "90.156.236.101", "90.156.236.102", "90.156.236.103", "90.156.236.104", "90.156.236.105",
        "90.156.236.106", "90.156.236.107", "90.156.236.108", "90.156.236.114", "90.156.236.115",
        "90.156.236.117"
      ]
    },
    {
      id: "193_203_43_0_24",
      title: "193.203.43.0/24",
      cidr: "193.203.43.0/24",
      count: 48,
      ips: [
        "193.203.43.0", "193.203.43.1", "193.203.43.2", "193.203.43.3", "193.203.43.4",
        "193.203.43.6", "193.203.43.7", "193.203.43.8", "193.203.43.9", "193.203.43.10",
        "193.203.43.11", "193.203.43.12", "193.203.43.13", "193.203.43.14", "193.203.43.15",
        "193.203.43.16", "193.203.43.17", "193.203.43.18", "193.203.43.19", "193.203.43.21",
        "193.203.43.22", "193.203.43.23", "193.203.43.24", "193.203.43.25", "193.203.43.26",
        "193.203.43.27", "193.203.43.28", "193.203.43.29", "193.203.43.30", "193.203.43.31",
        "193.203.43.32", "193.203.43.33", "193.203.43.34", "193.203.43.35", "193.203.43.36",
        "193.203.43.37", "193.203.43.38", "193.203.43.39", "193.203.43.40", "193.203.43.41",
        "193.203.43.42", "193.203.43.43", "193.203.43.44", "193.203.43.45", "193.203.43.46",
        "193.203.43.47", "193.203.43.48"
      ]
    },
    {
      id: "185_180_200_0_22",
      title: "185.180.200.0/22",
      cidr: "185.180.200.0/22",
      count: 2,
      ips: ["185.180.202.14", "185.180.202.15"]
    }
  ];

  // Helper: Strip VK URLs down to the raw hash
  function stripVkUrl(raw) {
    if (!raw) return "";
    var trimmed = String(raw).trim();
    var joinMatch = trimmed.match(/\/join\/([a-zA-Z0-9_\-]+)/i);
    if (joinMatch) return joinMatch[1];
    var hashMatch = trimmed.match(/[?&]hash=([a-zA-Z0-9_\-]+)/i);
    if (hashMatch) return hashMatch[1];
    var cleaned = trimmed.replace(/^https?:\/\/[^\/]+\//i, "").replace(/[^a-zA-Z0-9_\-]/g, "");
    return cleaned;
  }

  // Helper: Parse csqtt:// link
  function parseCsqttLink(raw) {
    if (!raw || !raw.startsWith("csqtt://")) return null;
    try {
      var queryIdx = raw.indexOf("?");
      if (queryIdx === -1) return null;
      var params = new URLSearchParams(raw.slice(queryIdx + 1));
      var host = params.get("host") || "";
      var peer = params.get("peer") || "";
      var port = params.get("port") || "";
      var password = params.get("password") || "";
      var rawHashes = params.get("hashes") || "";
      var hashes = rawHashes.split(",").map(function (h) {
        return stripVkUrl(h);
      }).filter(function (h) {
        return h.length >= 16;
      });
      var vkToken = params.get("vk_access_token") || "";

      return {
        host: host,
        peer: peer || port,
        password: password,
        hashes: hashes,
        vkAccessToken: vkToken
      };
    } catch (_) {
      return null;
    }
  }

  // Normalizes list of hashes
  function normalizeHashes(hashesList) {
    var out = [];
    (hashesList || []).forEach(function (h) {
      var c = stripVkUrl(h);
      if (c && c.length >= 16 && out.indexOf(c) === -1) {
        out.push(c);
      }
    });
    return out.join(",");
  }

  // Mode helpers mirroring vk_mode_policy.rs
  function vkAuthModeForHashMode(hashMode, currentAuthMode) {
    if (hashMode === "auto_js") return "auto_js";
    if (hashMode === "auto_api") return "vkcalls";
    if (currentAuthMode === "auto_js") return "vkcalls";
    return currentAuthMode || "vkcalls";
  }

  function shouldConfirmAutoJsMode(currentMode, requestedMode, acknowledged) {
    return !acknowledged && currentMode !== "auto_js" && requestedMode === "auto_js";
  }

  function shouldConfirmTcpTransport(currentTransport, requestedTransport, acknowledged) {
    return !acknowledged && currentTransport !== "tcp_tls" && requestedTransport === "tcp_tls";
  }

  function applyAppTheme(theme) {
    var t = theme || "system";
    document.documentElement.dataset.theme = t;
    document.documentElement.setAttribute("data-theme", t);
  }

  // ---------------------------------------------------------------------------
  // Dialogs
  // ---------------------------------------------------------------------------

  // Timed warning dialog (8-second countdown) for experimental / risk modes
  function showTimedWarningDialog(options) {
    var title = options.title || "Внимание!";
    var paragraphs = options.paragraphs || [];
    var onAcknowledge = options.onAcknowledge;
    var onDoNotRemind = options.onDoNotRemind;

    var secondsLeft = 8;
    var timerId = null;

    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px" }
    });

    paragraphs.forEach(function (text) {
      var p = window.UI.h("div", {
        className: "body-small text-muted",
        style: { lineHeight: "17px" },
        text: text
      });
      content.appendChild(p);
    });

    var timerBadge = window.UI.h("div", {
      className: "label-large text-primary",
      style: { textAlign: "center", minHeight: "22px", marginTop: "4px" },
      text: String(secondsLeft)
    });
    content.appendChild(timerBadge);

    var btnRow = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "8px", marginTop: "6px" }
    });

    var doNotRemindBtn = null;
    if (onDoNotRemind) {
      doNotRemindBtn = window.UI.button({
        text: "Не напоминать",
        variant: "outlined",
        fullWidth: true,
        enabled: false,
        onClick: function () {
          if (timerId) clearInterval(timerId);
          dlg.close();
          onDoNotRemind();
        }
      });
      btnRow.appendChild(doNotRemindBtn);
    }

    var ackBtn = window.UI.button({
      text: "Понятно",
      variant: "filled",
      fullWidth: true,
      enabled: false,
      onClick: function () {
        if (timerId) clearInterval(timerId);
        dlg.close();
        if (onAcknowledge) onAcknowledge();
      }
    });
    btnRow.appendChild(ackBtn);
    content.appendChild(btnRow);

    var dlg = window.UI.dialog({
      title: title,
      content: content,
      actions: []
    });

    timerId = setInterval(function () {
      secondsLeft -= 1;
      if (secondsLeft <= 0) {
        clearInterval(timerId);
        timerBadge.textContent = "";
        timerBadge.style.display = "none";
        ackBtn.disabled = false;
        if (doNotRemindBtn) doNotRemindBtn.disabled = false;
      } else {
        timerBadge.textContent = String(secondsLeft);
      }
    }, 1000);
  }

  function showAutoVkRiskDialog(onConfirm, onDoNotRemind) {
    showTimedWarningDialog({
      title: "Внимание!",
      paragraphs: [
        "«Авто ВК» использует неофициальные браузерные методы VK API для создания звонка и получения TURN credentials без анонимных звонков. Сетевой стек PRIMP маскирует HTTP/TLS-профиль под обычный браузер.",
        "Разработчик не может исключить блокировку VK-аккаунта и честно оценить её вероятность. Используйте режим на свой риск; если важен основной аккаунт, выбирайте второстепенный. Рекомендуется включать «Авто ВК», когда «Авто» и «Авто API» не работают."
      ],
      onAcknowledge: onConfirm,
      onDoNotRemind: onDoNotRemind
    });
  }

  function showTcpTransportRiskDialog(onConfirm, onDoNotRemind) {
    showTimedWarningDialog({
      title: "Внимание!",
      paragraphs: [
        "TCP — экспериментальный режим для сетей, в которых провайдер ограничивает UDP-трафик. TCP используется только между вашим устройством и серверами VK TURN; между VK TURN и вашим VPS трафик всегда идёт по UDP.",
        "Если UDP работает без ограничений, используйте его: UDP стабильнее, быстрее и по своей природе ближе к настоящим звонкам."
      ],
      onAcknowledge: onConfirm,
      onDoNotRemind: onDoNotRemind
    });
  }

  function showWorkersInfoDialog() {
    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "8px" }
    });
    var lines = [
      "• Каждый поток TURN имеет скорость около 2 Мбит/с.",
      "• Имейте в виду: лучше выбирать количество потоков исходя из реальной скорости вашего интернет-подключения, а не больше его максимума:",
      "  — 10 потоков: для скорости 20 Мбит/с",
      "  — 20 потоков: для скорости 40 Мбит/с",
      "  — 30 потоков: для скорости 60 Мбит/с",
      "  — и далее по нарастающей."
    ];
    lines.forEach(function (l) {
      content.appendChild(window.UI.h("div", {
        className: "body-small text-muted",
        style: { whiteSpace: "pre-wrap", lineHeight: "17px" },
        text: l
      }));
    });

    window.UI.dialog({
      title: "Настройка потоков",
      content: content,
      actions: [
        { text: "Понятно", variant: "filled", onClick: function (d) { d.close(); } }
      ]
    });
  }

  function showHashModeInfoDialog(mode) {
    var title = "Режимы ВК Хешей";
    var text = "";

    if (mode === "manual") {
      title = "Режим «Ручной»";
      text = "• Вы сами создаёте звонки или группы с вечными звонками в VK, копируете ссылку и вставляете её в окно «VK Хеши».\n" +
             "• Можно использовать до 6 разных хешей для распределения нагрузки.\n" +
             "• Полный ручной контроль над источниками пула звонков без необходимости входа в VK ID.";
    } else if (mode === "auto_api") {
      title = "Режим «Авто API»";
      text = "• Приложение создаёт необходимые звонки через официальный метод VK API calls.start.\n" +
             "• Количество хешей выбирается автоматически — от 1 до 6 по числу потоков.\n" +
             "• При остановке туннеля звонки корректно завершаются через calls.forceFinish.\n" +
             "• VK access token хранится в защищённом хранилище.";
    } else if (mode === "auto_js") {
      title = "Режим «Авто ВК»";
      text = "• Rust повторяет браузерную цепочку VK Calls: получает call-token через messages.getCallToken, открывает сессию OK Calls и создаёт один звонок через vchat.startConversation.\n" +
             "• session_key и идентификаторы звонков остаются внутри Rust-процесса и не выводятся в журнал.\n" +
             "• Источник TURN-кредов задаётся отдельно режимом работы: «Капча», «Авто» или account-сессии «Авто ВК».\n" +
             "• В режимах работы «Капча» и «Авто» создатель выходит после готовности TURN-наборов; в режиме «Авто ВК» удерживает звонок до отключения.\n" +
             "• WSS и WebTransport не запускаются, поскольку для получения join_link и TURN-хешей они не требуются.";
    } else {
      text = "• Ручной — несмотря на жёлтый индикатор, является стабильным и проверенно рабочим, но создавать вечные хеши получается сложным или невозможным.\n\n" +
             "• Авто API — использует вечный VK токен для создания звонков через официальный API calls.start, работает стабильно и хорошо. Создаёт звонки и использует их; после того как вы отключаетесь от CSQTT, звонки завершаются, ничего не утекает, поведение выглядит штатным.\n\n" +
             "• Авто ВК — использует браузерную цепочку API (методы calls.getConversationParams), также требует ваш VK токен. Стабильный метод на случай возникновения проблем с первыми двумя. Когда вы отключаетесь от CSQTT, звонок завершается.";
    }

    var content = window.UI.h("div", {
      className: "body-small text-muted",
      style: { whiteSpace: "pre-wrap", lineHeight: "18px" },
      text: text
    });

    window.UI.dialog({
      title: title,
      content: content,
      actions: [
        { text: "Понятно", variant: "filled", onClick: function (d) { d.close(); } }
      ]
    });
  }

  function showWorkModeInfoDialog(mode) {
    var title = "Режимы работы";
    var text = "";

    if (mode === "legacy") {
      title = "Режим «Капча»";
      text = "• Старый метод получения credentials через эмуляцию браузерного клиента.\n" +
             "• При таком способе VK может потребовать прохождение капчи.\n" +
             "• Если решить её автоматически не получится, приложение запросит ручное подтверждение в WebView.";
    } else if (mode === "vkcalls") {
      title = "Режим «Авто»";
      text = "• Получение TURN-сессий напрямую через протокол и гостевую цепочку VK Calls.\n" +
             "• Обычно позволяет работать без капчи.\n" +
             "• Поддерживает масштабирование до 100 независимых воркеров.";
    } else if (mode === "auto_js") {
      title = "Режим «Авто ВК»";
      text = "• Один аккаунт создаёт и удерживает один звонок через метод vchat.startConversation.\n" +
             "• Calls-сессии обслуживают группы потоков с общим максимумом 100 воркеров.\n" +
             "• Получает credentials через официальный метод get_conversation_params под авторизованным аккаунтом VK.\n" +
             "• Анонимные звонки не используются. Режим хешей автоматически фиксируется на «Авто ВК».\n" +
             "• Создатель удерживает звонок до отключения туннеля.\n" +
             "• Если отдельная сессия не выдаст креды, её группа перейдёт на «Авто» без капчи.";
    } else {
      text = "• Капча — считается первым и самым старым методом получения credentials, не советуется к штатному использованию.\n\n" +
             "• Авто — уже очень хороший гостевой режим через VK Calls, почти всегда не требует капчи и работает без проблем (до 100 потоков).\n\n" +
             "• Авто ВК — новый режим получения credentials через get_conversation_params. Использует залогиненный аккаунт ВК, анонимные звонки не используются. Советуем использовать этот режим, когда с режимом «Авто» наблюдаются проблемы. Режим самый стабильный, работает только в паре с Авто ВК методом получения ВК хешей.";
    }

    var content = window.UI.h("div", {
      className: "body-small text-muted",
      style: { whiteSpace: "pre-wrap", lineHeight: "18px" },
      text: text
    });

    window.UI.dialog({
      title: title,
      content: content,
      actions: [
        { text: "Понятно", variant: "filled", onClick: function (d) { d.close(); } }
      ]
    });
  }

  function showTurnTransportInfoDialog(mode) {
    var title = "Транспорт TURN";
    var text = "";

    if (mode === "udp") {
      title = "Транспорт UDP";
      text = "UDP — обычный и рекомендуемый путь от устройства до VK TURN. Он даёт наименьшую задержку, не создаёт очередь TCP и лучше подходит для игр, звонков и коротких пакетов.\n\n" +
             "Выбирайте его по умолчанию. Между VK TURN и вашим VPS также всегда используется UDP.";
    } else if (mode === "tcp_tls") {
      title = "Транспорт TCP";
      text = "Этот режим нужен только если оператор режет или блокирует UDP, например в отдельных сетях Ростелекома.\n\n" +
             "TCP действует только на участке между вашим устройством и выданным VK TURN-узлом. Между VK TURN и вашим VPS трафик всегда передаётся по UDP.\n\n" +
             "Для обычного TURN-адреса это не TLS; если VK когда-либо выдаст turns-адрес, он будет использовать TLS. На UDP этот режим автоматически не переключается. TCP может увеличить ping и просадки при нагрузке из-за очереди TCP.";
    } else {
      text = "По умолчанию CSQTT использует UDP: это лучший выбор по скорости, задержке и отзывчивости.\n\n" +
             "TCP — отдельный режим для сетей, где UDP ограничен. Он влияет только на путь устройства до VK TURN; участок между VK TURN и VPS всегда использует UDP. В TCP-режиме нет скрытого перехода на UDP на участке устройства до TURN.\n\n" +
             "Изменение применяется при следующем подключении.";
    }

    var content = window.UI.h("div", {
      className: "body-small text-muted",
      style: { whiteSpace: "pre-wrap", lineHeight: "18px" },
      text: text
    });

    window.UI.dialog({
      title: title,
      content: content,
      actions: [
        { text: "Понятно", variant: "filled", onClick: function (d) { d.close(); } }
      ]
    });
  }

  function showWifiAutoPauseInfoDialog() {
    var content = window.UI.h("div", {
      className: "body-small text-muted",
      style: { whiteSpace: "pre-wrap", lineHeight: "18px" },
      text: "Автопауза при Wi-Fi позволяет автоматически приостанавливать VPN-туннель при подключении к доверенной домашней или рабочей сети Wi-Fi, экономя трафик и ресурсы сервера.\n\n" +
            "При возвращении на мобильную сеть туннель автоматически возобновляет работу."
    });

    window.UI.dialog({
      title: "Автопауза при Wi-Fi",
      content: content,
      actions: [
        { text: "Понятно", variant: "filled", onClick: function (d) { d.close(); } }
      ]
    });
  }

  // Dialog: VK Hashes (up to 6 hashes, paste, format validation, active check)
  function showHashesDialog(initialHashesString, onSaved) {
    var parts = (initialHashesString || "")
      .split(/[\s,\n]+/)
      .map(stripVkUrl)
      .filter(function (x) { return !!x; });

    var hashes = [
      parts[0] || "",
      parts[1] || "",
      parts[2] || "",
      parts[3] || "",
      parts[4] || "",
      parts[5] || ""
    ];

    var container = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px", width: "100%", maxWidth: "460px" }
    });

    var subtitle = window.UI.h("div", {
      className: "body-small text-muted",
      style: { lineHeight: "16px" },
      text: "Больше хешей — выше лимит потоков и лучшее распределение нагрузки."
    });
    container.appendChild(subtitle);

    var fieldsContainer = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "8px" }
    });
    container.appendChild(fieldsContainer);

    var fieldObjs = [];

    function renderFields() {
      fieldsContainer.innerHTML = "";
      fieldObjs = [];

      hashes.forEach(function (val, idx) {
        var label = (idx === 0) ? "VK Хеш 1 *" : ("VK Хеш " + (idx + 1));

        var trailing = window.UI.h("div", {
          style: { display: "flex", alignItems: "center", gap: "2px" }
        });

        // Paste button
        var pasteBtn = window.UI.iconButton({
          icon: "ContentCopy",
          title: "Вставить из буфера",
          size: 16,
          onClick: function () {
            if (navigator.clipboard && navigator.clipboard.readText) {
              navigator.clipboard.readText().then(function (text) {
                if (text) {
                  var cleaned = stripVkUrl(text);
                  hashes[idx] = cleaned;
                  renderFields();
                }
              }).catch(function () {
                window.UI.toast("Не удалось прочитать буфер");
              });
            } else {
              window.UI.toast("Буфер обмена недоступен");
            }
          }
        });
        trailing.appendChild(pasteBtn);

        // Delete button
        if (val) {
          var delBtn = window.UI.iconButton({
            icon: "Delete",
            title: "Очистить",
            size: 16,
            onClick: function () {
              hashes[idx] = "";
              renderFields();
            }
          });
          trailing.appendChild(delBtn);
        }

        var isShort = val.length > 0 && val.length < 16;
        var errorMsg = isShort ? ("Хеш " + (idx + 1) + " — короткий (мин. 16)") : null;

        var f = window.UI.textField({
          label: label,
          placeholder: "Ссылка звонка или хеш",
          value: val,
          error: errorMsg,
          trailing: trailing,
          onInput: function (newVal) {
            hashes[idx] = stripVkUrl(newVal);
          }
        });
        fieldObjs.push(f);
        fieldsContainer.appendChild(f.el);
      });
    }

    renderFields();

    var statusEl = window.UI.h("div", {
      className: "body-small text-muted",
      style: { minHeight: "18px", marginTop: "2px" }
    });
    container.appendChild(statusEl);

    // "Проверить актив" button
    var checkBtn = window.UI.button({
      text: "Проверить актив",
      variant: "outlined",
      fullWidth: true,
      onClick: function () {
        var active = hashes.map(function (h) { return stripVkUrl(h); }).filter(function (h) { return h.length >= 16; });
        if (active.length === 0) {
          statusEl.textContent = "Нет валидных хешей для проверки";
          statusEl.className = "body-small text-error";
          return;
        }

        // Check for duplicates
        var uniq = [];
        var hasDuplicates = false;
        active.forEach(function (h) {
          if (uniq.indexOf(h) !== -1) hasDuplicates = true;
          else uniq.push(h);
        });

        if (hasDuplicates) {
          statusEl.textContent = "Обнаружены дубликаты хешей!";
          statusEl.className = "body-small text-error";
          return;
        }

        statusEl.textContent = "Проверка формата: " + uniq.length + " валидных хеш(ей) ✓";
        statusEl.className = "body-small text-primary";
      }
    });
    container.appendChild(checkBtn);

    var dlg = window.UI.dialog({
      title: "VK Хеши",
      content: container,
      actions: [
        { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
        {
          text: "Сохранить",
          variant: "filled",
          onClick: function (d) {
            var normalized = normalizeHashes(hashes);
            onSaved(normalized);
            d.close();
          }
        }
      ]
    });
  }

  // Dialog: Secrets (Connection password encrypted storage)
  function showSecretsDialog(onSaved) {
    var passInputObj = window.UI.textField({
      label: "Пароль туннеля",
      placeholder: "укажите пароль",
      value: "",
      isPassword: true,
      onInput: function (val) {
        validatePass(val);
      }
    });

    function validatePass(val) {
      if (!val) {
        passInputObj.setError(null);
        return true;
      }
      var validChars = /^[a-zA-Z0-9_.!?:#\/-]+$/;
      if (!validChars.test(val)) {
        passInputObj.setError("Разрешены только буквы, цифры и знаки . ! ? : # - _ /");
        return false;
      }
      passInputObj.setError(null);
      return true;
    }

    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px", minWidth: "280px" }
    });
    content.appendChild(passInputObj.el);

    var dlg = window.UI.dialog({
      title: "Подключение",
      content: content,
      actions: [
        { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
        {
          text: "Сохранить",
          variant: "filled",
          onClick: function (d) {
            var p = passInputObj.getValue().trim();
            if (!validatePass(p)) return;
            window.csqtt.setSecret(SECRET_CONNECTION_PASSWORD, p).then(function () {
              window.UI.toast("Пароль туннеля сохранён");
              if (onSaved) onSaved(p);
              d.close();
            }).catch(function (err) {
              window.UI.toast("Ошибка сохранения пароля: " + (err.message || err));
            });
          }
        }
      ]
    });
  }

  // Dialog: VK Token Revocation
  function showVkTokenRevokeDialog(onRevoked) {
    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "12px" }
    });

    var pText = window.UI.h("div", {
      className: "body-small text-muted",
      style: { lineHeight: "17px" },
      text: "При удалении токена сессия VK сохраняется настолько, насколько это возможно: токен можно будет быстро получить заново, либо вы сможете сменить аккаунт самостоятельно."
    });
    content.appendChild(pText);

    var copyBtn = window.UI.button({
      text: "Копировать токен",
      variant: "outlined",
      fullWidth: true,
      onClick: function () {
        window.csqtt.exportJson(true).then(function (rawJson) {
          try {
            var data = typeof rawJson === "string" ? JSON.parse(rawJson) : rawJson;
            var token = (data.secrets && data.secrets.vk_access_token) || data.vk_access_token || "";
            if (token && navigator.clipboard && navigator.clipboard.writeText) {
              navigator.clipboard.writeText(token);
              window.UI.toast("Бессрочный VK токен скопирован");
            } else {
              window.UI.toast("VK токен скопирован");
            }
          } catch (_) {
            window.UI.toast("Не удалось прочитать токен");
          }
        }).catch(function () {
          window.UI.toast("Не удалось скопировать токен");
        });
      }
    });
    content.appendChild(copyBtn);

    var revokeBtn = window.UI.button({
      text: "Удалить токен",
      variant: "outlined",
      fullWidth: true,
      onClick: function () {
        window.csqtt.setSecret(SECRET_VK_TOKEN, "").then(function () {
          return window.App.saveSettings(function (s) {
            var prof = s.profiles[s.active_profile || 0];
            if (prof) prof.vk_access_token_user_id = "";
          });
        }).then(function () {
          window.UI.toast("VK токен удалён");
          if (onRevoked) onRevoked();
          dlg.close();
        }).catch(function (err) {
          window.UI.toast("Ошибка удаления: " + (err.message || err));
        });
      }
    });
    content.appendChild(revokeBtn);

    var dlg = window.UI.dialog({
      title: "Отзыв VK-авторизации",
      content: content,
      actions: [
        { text: "Назад", variant: "filled", onClick: function (d) { d.close(); } }
      ]
    });
  }

  // Dialog: Turn Subnets Selection mirroring TurnSubnetsDialog.kt
  function showTurnSubnetsDialog(currentSubnetId, currentIpLimit, onSaved) {
    var selectedId = currentSubnetId || "";
    var ipLimit = currentIpLimit || "ALL";

    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "12px", maxHeight: "65vh" }
    });

    var desc = window.UI.h("div", {
      className: "body-small text-muted",
      style: { lineHeight: "16px" },
      text: "Выберите подсеть для распределения потоков. Если все выключены — используются адреса по умолчанию из ответа VK."
    });
    content.appendChild(desc);

    var limitRow = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }
    });
    limitRow.appendChild(window.UI.h("span", { className: "label-medium text-muted", text: "Лимит адресов в пуле" }));

    var limitSeg = window.UI.segmented({
      options: [
        { key: "ALL", label: "Все" },
        { key: "10", label: "≥10" },
        { key: "20", label: "≥20" },
        { key: "50", label: "≥50" }
      ],
      selected: ipLimit,
      onSelected: function (key) {
        ipLimit = key;
        renderSubnetsList();
      }
    });
    limitRow.appendChild(limitSeg);
    content.appendChild(limitRow);

    var listWrap = window.UI.h("div", {
      className: "dialog-content",
      style: { display: "flex", flexDirection: "column", gap: "6px", maxHeight: "320px", overflowY: "auto" }
    });
    content.appendChild(listWrap);

    function renderSubnetsList() {
      listWrap.innerHTML = "";
      var limitNum = parseInt(ipLimit, 10);

      TURN_SUBNETS.forEach(function (sub) {
        var isEligible = isNaN(limitNum) || sub.count >= limitNum;
        var isChecked = isEligible && (selectedId === sub.id);

        var card = window.UI.h("div", {
          className: "setting-row" + (!isEligible ? " disabled" : ""),
          style: {
            padding: "8px 12px",
            borderRadius: "16px",
            border: "1px solid " + (isChecked ? "var(--md-sys-color-primary)" : "var(--md-sys-color-outline-variant)"),
            backgroundColor: isChecked ? "var(--md-sys-color-primary-container)" : "transparent",
            cursor: isEligible ? "pointer" : "default"
          }
        });

        var textCol = window.UI.h("div", { className: "setting-row-text" });
        var titleSpan = window.UI.h("span", {
          className: "body-medium",
          style: { fontWeight: isChecked ? "700" : "500" },
          text: sub.title
        });
        var countSpan = window.UI.h("span", {
          className: "label-small text-muted",
          text: sub.count + " серверов в пуле"
        });
        textCol.appendChild(titleSpan);
        textCol.appendChild(countSpan);
        card.appendChild(textCol);

        var sw = window.UI.switchWidget({
          checked: isChecked,
          enabled: isEligible,
          onChange: function (c) {
            selectedId = c ? sub.id : "";
            renderSubnetsList();
          }
        });
        card.appendChild(sw);

        if (isEligible) {
          card.addEventListener("click", function (e) {
            if (e.target.closest(".m3-switch")) return;
            selectedId = (selectedId === sub.id) ? "" : sub.id;
            renderSubnetsList();
          });
        }

        listWrap.appendChild(card);
      });
    }

    renderSubnetsList();

    var dlg = window.UI.dialog({
      title: "Выбор TURN подсетей",
      content: content,
      actions: [
        { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
        {
          text: "Сохранить",
          variant: "filled",
          onClick: function (d) {
            onSaved(selectedId, ipLimit);
            d.close();
          }
        }
      ]
    });
  }

  // Dialog: Rename Profile
  function showEditProfileNameDialog(profileIndex, currentName, onSaved) {
    var inputObj = window.UI.textField({
      label: "Название профиля",
      value: currentName || ("Профиль " + profileIndex),
      singleLine: true
    });

    var content = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px", minWidth: "280px" }
    });
    content.appendChild(inputObj.el);

    var dlg = window.UI.dialog({
      title: "Название профиля",
      content: content,
      actions: [
        { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
        {
          text: "Сохранить",
          variant: "filled",
          onClick: function (d) {
            var name = inputObj.getValue().trim();
            onSaved(profileIndex, name);
            d.close();
          }
        }
      ]
    });
  }

  // ---------------------------------------------------------------------------
  // Main Screen Renderer
  // ---------------------------------------------------------------------------

  function renderSettingsScreen(container) {
    container.innerHTML = "";

    var saveDebounceTimer = null;
    function scheduleSave(mutator) {
      clearTimeout(saveDebounceTimer);
      saveDebounceTimer = setTimeout(function () {
        window.App.saveSettings(mutator);
      }, 300);
    }

    var root = window.UI.h("div", {
      className: "screen-inner",
      style: { display: "flex", flexDirection: "column", gap: "var(--spacing-md)", width: "100%", flex: "1" }
    });
    container.appendChild(root);

    // =========================================================================
    // Card 1: Profiles & Role Management (mirroring Android FloatingToolbar)
    // =========================================================================
    var profileCard = window.UI.sectionCard();
    root.appendChild(profileCard);

    var roleLabel = window.UI.h("div", {
      className: "label-medium text-muted",
      text: "Ваша роль использования"
    });
    profileCard.appendChild(roleLabel);

    var roleSeg = window.UI.segmented({
      options: [
        { key: "owner", label: "Владелец" },
        { key: "participant", label: "Участник" }
      ],
      selected: "owner",
      onSelected: function (role) {
        var isPart = (role === "participant");
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.csqtt_link_mode = isPart;
        });
      }
    });
    profileCard.appendChild(roleSeg);
    profileCard.appendChild(window.UI.divider());

    // Profile Dropdown with edit button
    var profileDropdownWrap = window.UI.h("div", {
      style: { display: "flex", alignItems: "flex-end", gap: "8px", width: "100%" }
    });

    var profileDropdownHolder = window.UI.h("div", { style: { flex: "1" } });
    profileDropdownWrap.appendChild(profileDropdownHolder);

    var editProfileBtn = window.UI.button({
      text: "Имя",
      variant: "outlined",
      onClick: function () {
        var s = window.App.state.settings;
        if (!s) return;
        var pIdx = s.active_profile || 0;
        var prof = s.profiles[pIdx] || {};
        var curName = prof.profile_name || ("Профиль " + pIdx);
        showEditProfileNameDialog(pIdx, curName, function (idx, newName) {
          window.App.saveSettings(function (st) {
            if (st.profiles[idx]) st.profiles[idx].profile_name = newName;
          });
        });
      }
    });
    editProfileBtn.style.height = "44px";
    profileDropdownWrap.appendChild(editProfileBtn);

    var resetProfileBtn = window.UI.button({
      text: "Сброс",
      variant: "outlined",
      onClick: function () {
        var s = window.App.state.settings;
        if (!s) return;
        var pIdx = s.active_profile || 0;
        window.UI.confirm({
          title: "Сбросить профиль?",
          message: "Все настройки профиля " + pIdx + " будут сброшены к значениям по умолчанию.",
          confirmText: "Сбросить"
        }).then(function (ok) {
          if (!ok) return;
          window.App.saveSettings(function (st) {
            if (st.profiles[pIdx]) {
              st.profiles[pIdx] = {
                profile_name: "",
                peer: "",
                vk_hashes: "",
                secondary_vk_hash: "",
                workers_per_hash: 18,
                protocol: "udp",
                listen_port: 0,
                manual_ports_enabled: false,
                server_peer_port: DEFAULT_SERVER_PEER_PORT,
                sni: "",
                no_dns: false,
                custom_turn_ips: "",
                selected_turn_subnet: "",
                turn_subnet_ip_limit: "ALL",
                user_agent: "",
                vk_auth_mode: "vkcalls",
                obfs_mode: "unified",
                turn_transport: "udp",
                srtp_mode: "auto",
                captcha_mode: "auto",
                captcha_solve_method: "auto",
                captcha_wbv_solve_method: "auto",
                vk_hash_mode: "manual",
                vk_access_token_user_id: "",
                selected_fingerprint: "chrome",
                active_client_ids: "8202606,6287487",
                vk_hash_check_results: "{}",
                csqtt_link: "",
                csqtt_link_mode: false,
                participant_show_logs: false
              };
            }
          }).then(function () {
            window.UI.toast("Профиль " + pIdx + " сброшен");
          });
        });
      }
    });
    resetProfileBtn.style.height = "44px";
    profileDropdownWrap.appendChild(resetProfileBtn);

    profileCard.appendChild(profileDropdownWrap);

    // Fingerprint Dropdown
    var fingerprintDropdown = window.UI.dropdown({
      title: "Отпечаток",
      selectedKey: "chrome",
      options: [
        { key: "chrome", label: "Chrome" },
        { key: "safari", label: "Safari" },
        { key: "firefox", label: "Firefox" },
        { key: "edge", label: "Edge" },
        { key: "opera", label: "Opera" }
      ],
      onSelected: function (fp) {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.selected_fingerprint = fp;
        });
      }
    });
    profileCard.appendChild(fingerprintDropdown);

    // Theme Mode Dropdown
    var themeDropdown = window.UI.dropdown({
      title: "Оформление",
      selectedKey: "system",
      options: [
        { key: "system", label: "Система" },
        { key: "light", label: "Светлая" },
        { key: "dark", label: "Тёмная" }
      ],
      onSelected: function (thm) {
        applyAppTheme(thm);
        window.App.saveSettings(function (s) {
          s.theme_mode = thm;
        });
      }
    });
    profileCard.appendChild(themeDropdown);

    // Wi-Fi Auto-Pause Row
    var autoPauseRow = window.UI.settingRow({
      title: "Автопауза при Wi-Fi",
      description: "Приостанавливать туннель при подключении к Wi-Fi",
      checked: false,
      onCheckedChange: function (checked) {
        window.App.saveSettings(function (s) {
          s.auto_pause_on_wifi = checked;
        });
      },
      onInfoClick: function () {
        showWifiAutoPauseInfoDialog();
      }
    });
    profileCard.appendChild(autoPauseRow);

    // Participant show logs toggle
    var participantLogsRow = window.UI.settingRow({
      title: "Показывать логи",
      description: "Отображать вкладку логов для участника",
      checked: false,
      onCheckedChange: function (checked) {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.participant_show_logs = checked;
        });
      }
    });
    profileCard.appendChild(participantLogsRow);

    // Manual Ports Mode Switch
    var manualPortsRow = window.UI.settingRow({
      title: "Ручные порты",
      description: "Разрешить явный ввод порта сервера (PEER)",
      checked: false,
      onCheckedChange: function (checked) {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.manual_ports_enabled = checked;
        });
      }
    });
    profileCard.appendChild(manualPortsRow);

    // Turn Subnets Dialog Button
    var turnSubnetsBtn = window.UI.button({
      text: "Выбор TURN подсетей",
      variant: "outlined",
      fullWidth: true,
      onClick: function () {
        var p = window.App.profile() || {};
        showTurnSubnetsDialog(p.selected_turn_subnet, p.turn_subnet_ip_limit, function (subId, limit) {
          window.App.saveSettings(function (s) {
            var prof = s.profiles[s.active_profile || 0];
            if (prof) {
              prof.selected_turn_subnet = subId;
              prof.turn_subnet_ip_limit = limit;
            }
          }).then(function () {
            window.UI.toast("Настройки подсетей TURN сохранены");
          });
        });
      }
    });
    profileCard.appendChild(turnSubnetsBtn);

    // =========================================================================
    // Card 2: Main Tunnel Settings Card (SettingsTab.kt)
    // =========================================================================
    var tunnelCard = window.UI.sectionCard();
    root.appendChild(tunnelCard);

    // Dynamic Server / Hashes Section Container
    var topConfigContainer = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px" }
    });
    tunnelCard.appendChild(topConfigContainer);

    // --- Participant Mode UI Elements ---
    var participantBox = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px" }
    });

    var linkInput = window.UI.textField({
      label: "Ссылка csqtt://",
      placeholder: "csqtt://connect?v=2&host=ip&peer=порт&password=пароль",
      value: "",
      onInput: function (val) {
        var cleaned = val.replace(/\s+/g, "");
        var parsed = parseCsqttLink(cleaned);
        if (cleaned && !parsed) {
          linkInput.setError("Неверная ссылка CSQTT v2");
        } else {
          linkInput.setError(null);
        }
        scheduleSave(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.csqtt_link = cleaned;
        });
      }
    });
    participantBox.appendChild(linkInput.el);

    var linkInfoBadge = window.UI.h("div", {
      className: "body-medium text-primary",
      style: { fontWeight: "600", paddingLeft: "4px", minHeight: "20px" }
    });
    participantBox.appendChild(linkInfoBadge);

    // --- Owner Mode UI Elements ---
    var ownerBox = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "10px" }
    });

    var ownerTitle = window.UI.h("div", {
      className: "title-medium",
      text: "Сервер и Хеши"
    });
    ownerBox.appendChild(ownerTitle);

    var serverInputsRow = window.UI.h("div", {
      style: { display: "flex", gap: "10px", width: "100%" }
    });
    ownerBox.appendChild(serverInputsRow);

    var peerField = window.UI.textField({
      label: "IP сервера или домен",
      placeholder: "1.2.3.4",
      value: "",
      onInput: function (val) {
        var cleaned = val.replace(/\s+/g, "");
        if (cleaned.indexOf(":") !== -1) {
          peerField.setError("Укажите IP или домен без двоеточия и порта");
        } else {
          peerField.setError(null);
        }
        scheduleSave(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.peer = cleaned;
        });
      }
    });
    peerField.el.style.flex = "1";
    serverInputsRow.appendChild(peerField.el);

    var peerPortField = window.UI.textField({
      label: "Порт PEER",
      placeholder: String(DEFAULT_SERVER_PEER_PORT),
      value: String(DEFAULT_SERVER_PEER_PORT),
      onInput: function (val) {
        var num = parseInt(val.replace(/\D/g, ""), 10);
        if (isNaN(num) || num < 1 || num > 65535) {
          peerPortField.setError("1..65535");
        } else {
          peerPortField.setError(null);
          scheduleSave(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) p.server_peer_port = num;
          });
        }
      }
    });
    peerPortField.el.style.maxWidth = "130px";
    serverInputsRow.appendChild(peerPortField.el);

    // --- Hashes Action Controls (shared) ---
    var hashesActionWrap = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "6px" }
    });

    // Manual mode button
    var openHashesBtn = window.UI.button({
      text: "Ссылки VK звонков",
      variant: "outlined",
      icon: "Phone",
      fullWidth: true,
      onClick: function () {
        var prof = window.App.profile() || {};
        showHashesDialog(prof.vk_hashes, function (savedHashes) {
          window.App.saveSettings(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) p.vk_hashes = savedHashes;
          }).then(function () {
            window.UI.toast("VK хеши сохранены");
          });
        });
      }
    });
    hashesActionWrap.appendChild(openHashesBtn);

    // Active session button
    var activeSessionBtn = window.UI.button({
      text: "Сессия VK активна",
      variant: "outlined",
      icon: "CheckCircle",
      fullWidth: true,
      onClick: function () {
        showVkTokenRevokeDialog();
      }
    });
    hashesActionWrap.appendChild(activeSessionBtn);

    // Login button
    var vkLoginBtn = window.UI.button({
      text: "Вход VK",
      variant: "filled",
      icon: "Login",
      fullWidth: true,
      onClick: function () {
        if (window.Dialogs && window.Dialogs.showVkLoginDialog) {
          window.Dialogs.showVkLoginDialog();
        } else {
          window.csqtt.runSelf(["vk-login"], {
            onOutput: function (t) { console.log("vk-login:", t); }
          });
        }
      }
    });
    hashesActionWrap.appendChild(vkLoginBtn);

    // Hash Mode Dropdown
    var hashModeDropdown = window.UI.dropdown({
      title: "Режим хешей",
      selectedKey: "manual",
      options: [
        { key: "manual", label: "Ручной" },
        { key: "auto_api", label: "Авто API" },
        { key: "auto_js", label: "Авто ВК" }
      ],
      indicatorProvider: function (mode) {
        if (mode === "manual") return { progress: 0.78, color: "#43A047" };
        if (mode === "auto_api") return { progress: 1.0, color: "#1475FD" };
        if (mode === "auto_js") return { progress: 1.0, color: "#1475FD" };
        return null;
      },
      onTitleInfo: function () { showHashModeInfoDialog(null); },
      onInfo: function (m) { showHashModeInfoDialog(m); },
      onSelected: function (mode) {
        var prof = window.App.profile() || {};
        var curAuth = prof.vk_auth_mode || "vkcalls";
        var nextAuth = vkAuthModeForHashMode(mode, curAuth);
        var ack = window.App.state.settings && window.App.state.settings.auto_js_risk_acknowledged;

        function applySelection() {
          window.App.saveSettings(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) {
              p.vk_hash_mode = mode;
              p.vk_auth_mode = nextAuth;
            }
          });
        }

        if (shouldConfirmAutoJsMode(curAuth, nextAuth, ack)) {
          showAutoVkRiskDialog(
            applySelection,
            function () {
              window.App.saveSettings(function (s) {
                s.auto_js_risk_acknowledged = true;
              });
              applySelection();
            }
          );
        } else {
          applySelection();
        }
      }
    });

    topConfigContainer.appendChild(participantBox);
    topConfigContainer.appendChild(ownerBox);
    topConfigContainer.appendChild(hashesActionWrap);
    topConfigContainer.appendChild(hashModeDropdown);

    // --- Common Tunnel Controls ---
    var workModeDropdown = window.UI.dropdown({
      title: "Режим кредов",
      selectedKey: "vkcalls",
      options: [
        { key: "legacy", label: "Капча" },
        { key: "vkcalls", label: "Авто" },
        { key: "auto_js", label: "Авто ВК" }
      ],
      indicatorProvider: function (mode) {
        if (mode === "legacy") return { progress: 0.30, color: "#E53935" };
        if (mode === "vkcalls") return { progress: 0.78, color: "#43A047" };
        if (mode === "auto_js") return { progress: 1.0, color: "#1475FD" };
        return null;
      },
      onTitleInfo: function () { showWorkModeInfoDialog(null); },
      onInfo: function (m) { showWorkModeInfoDialog(m); },
      onSelected: function (mode) {
        var prof = window.App.profile() || {};
        var curAuth = prof.vk_auth_mode || "vkcalls";
        var ack = window.App.state.settings && window.App.state.settings.auto_js_risk_acknowledged;

        function applySelection() {
          window.App.saveSettings(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) p.vk_auth_mode = mode;
          });
        }

        if (shouldConfirmAutoJsMode(curAuth, mode, ack)) {
          showAutoVkRiskDialog(
            applySelection,
            function () {
              window.App.saveSettings(function (s) {
                s.auto_js_risk_acknowledged = true;
              });
              applySelection();
            }
          );
        } else {
          applySelection();
        }
      }
    });
    tunnelCard.appendChild(workModeDropdown);

    var srtpModeDropdown = window.UI.dropdown({
      title: "Режим SRTP",
      selectedKey: "auto",
      options: [
        { key: "wrap-srtp", label: "WRAP-SRTP" },
        { key: "dtls-srtp", label: "DTLS-SRTP" },
        { key: "auto", label: "Авто" }
      ],
      indicatorProvider: function (mode) {
        if (mode === "wrap-srtp") return { progress: 0.5, color: "#43A047" };
        if (mode === "dtls-srtp") return { progress: 1.0, color: "#43A047" };
        if (mode === "auto") return { progress: 1.0, color: "#1475FD" };
        return null;
      },
      onSelected: function (mode) {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.srtp_mode = mode;
        });
      }
    });
    tunnelCard.appendChild(srtpModeDropdown);

    var turnTransportDropdown = window.UI.dropdown({
      title: "Транспорт",
      selectedKey: "udp",
      options: [
        { key: "tcp_tls", label: "TCP" },
        { key: "udp", label: "UDP" }
      ],
      indicatorProvider: function (t) {
        if (t === "udp") return { progress: 1.0, color: "#1475FD" };
        if (t === "tcp_tls") return { progress: 0.70, color: "#43A047" };
        return null;
      },
      onTitleInfo: function () { showTurnTransportInfoDialog(null); },
      onInfo: function (m) { showTurnTransportInfoDialog(m); },
      onSelected: function (transport) {
        var prof = window.App.profile() || {};
        var cur = prof.turn_transport || "udp";
        var ack = window.App.state.settings && window.App.state.settings.tcp_transport_risk_acknowledged;

        function applySelection() {
          window.App.saveSettings(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) p.turn_transport = transport;
          });
        }

        if (shouldConfirmTcpTransport(cur, transport, ack)) {
          showTcpTransportRiskDialog(
            applySelection,
            function () {
              window.App.saveSettings(function (s) {
                s.tcp_transport_risk_acknowledged = true;
              });
              applySelection();
            }
          );
        } else {
          applySelection();
        }
      }
    });
    tunnelCard.appendChild(turnTransportDropdown);

    // Workers Slider
    var workersSlider = window.UI.slider({
      min: MIN_WORKERS,
      max: MAX_WORKERS,
      step: WORKERS_STEP,
      value: MIN_WORKERS,
      label: "Потоки TURN (воркеры)",
      valueFormatter: function (v) {
        return v + " (" + (v * 2) + " Мбит/с)";
      },
      onInfo: function () {
        showWorkersInfoDialog();
      },
      onValueChange: function (val) {
        var rounded = roundWorkersToStep(val);
        scheduleSave(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.workers_per_hash = rounded;
        });
      }
    });
    tunnelCard.appendChild(workersSlider);

    // --- Captcha Section (visible only when vk_auth_mode === 'legacy') ---
    var captchaSection = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "8px" }
    });
    captchaSection.appendChild(window.UI.divider());

    var autoCaptchaRow = window.UI.settingRow({
      title: "Авто капча",
      description: "Решать капчу автоматически",
      checked: true,
      onCheckedChange: function (checked) {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (!p) return;
          if (checked) {
            p.captcha_mode = "auto";
            p.captcha_solve_method = "auto";
          } else {
            p.captcha_mode = "wv";
            p.captcha_solve_method = "manual";
          }
        });
      }
    });
    captchaSection.appendChild(autoCaptchaRow);

    var captchaManualOptions = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "8px" }
    });

    var methodRow = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }
    });
    methodRow.appendChild(window.UI.h("span", { className: "body-medium", text: "Метод обхода капчи" }));
    var methodChipsWrap = window.UI.h("div", { style: { display: "flex", gap: "6px" } });

    var wbvChip = window.UI.chip({
      label: "WBV",
      selected: true,
      onClick: function () {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) {
            p.captcha_mode = "wv";
            p.captcha_solve_method = "manual";
          }
        });
      }
    });
    var rjsChip = window.UI.chip({
      label: "RJS",
      selected: false,
      onClick: function () {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) {
            p.captcha_mode = "rjs";
            p.captcha_solve_method = "auto";
          }
        });
      }
    });
    methodChipsWrap.appendChild(wbvChip);
    methodChipsWrap.appendChild(rjsChip);
    methodRow.appendChild(methodChipsWrap);
    captchaManualOptions.appendChild(methodRow);

    var solveRow = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }
    });
    solveRow.appendChild(window.UI.h("span", { className: "body-medium", text: "Режим обхода" }));
    var solveChipsWrap = window.UI.h("div", { style: { display: "flex", gap: "6px" } });

    var solveManualChip = window.UI.chip({
      label: "РУЧ",
      selected: true,
      onClick: function () {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) {
            p.captcha_wbv_solve_method = "manual";
            p.captcha_solve_method = "manual";
          }
        });
      }
    });
    var solveAutoChip = window.UI.chip({
      label: "АВТ",
      selected: false,
      onClick: function () {
        window.App.saveSettings(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) {
            p.captcha_wbv_solve_method = "auto";
            p.captcha_solve_method = "auto";
          }
        });
      }
    });
    solveChipsWrap.appendChild(solveManualChip);
    solveChipsWrap.appendChild(solveAutoChip);
    solveRow.appendChild(solveChipsWrap);
    captchaManualOptions.appendChild(solveRow);

    captchaSection.appendChild(captchaManualOptions);
    tunnelCard.appendChild(captchaSection);

    // SNI Field
    var sniField = window.UI.textField({
      label: "SNI (опционально)",
      placeholder: "например yandex.ru",
      value: "",
      onInput: function (val) {
        scheduleSave(function (s) {
          var p = s.profiles[s.active_profile || 0];
          if (p) p.sni = val.trim();
        });
      }
    });
    tunnelCard.appendChild(sniField.el);

    // Authorization Key Button (Connection Password)
    var authSectionWrap = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "6px", width: "100%" }
    });
    root.appendChild(authSectionWrap);

    var authPasswordBtn = window.UI.button({
      text: "Авторизация",
      variant: "outlined",
      icon: "Key",
      fullWidth: true,
      onClick: function () {
        showSecretsDialog();
      }
    });
    authSectionWrap.appendChild(authPasswordBtn);

    // =========================================================================
    // Dynamic State Refresh
    // =========================================================================
    function syncUI(st) {
      if (!st || !st.settings) return;
      var s = st.settings;
      var pIdx = s.active_profile || 0;
      var p = s.profiles[pIdx] || {};
      var isTunnelRunning = st.status && (st.status.state === "connected" || st.status.state === "starting");

      // 1. Theme
      if (s.theme_mode) {
        applyAppTheme(s.theme_mode);
        themeDropdown.setSelectedKey(s.theme_mode);
      }

      // 2. Wi-Fi Auto-Pause
      autoPauseRow.setChecked(!!s.auto_pause_on_wifi);

      // 3. Profiles dropdown
      profileDropdownHolder.innerHTML = "";
      var profOptions = (s.profiles || []).map(function (prof, idx) {
        return {
          key: String(idx),
          label: (prof.profile_name && prof.profile_name.trim()) ? prof.profile_name.trim() : ("Профиль " + idx)
        };
      });

      var profDropdown = window.UI.dropdown({
        title: "Профиль конфигурации",
        selectedKey: String(pIdx),
        options: profOptions,
        onSelected: function (key) {
          var next = parseInt(key, 10);
          window.App.saveSettings(function (setObj) {
            setObj.active_profile = next;
          });
        }
      });
      profileDropdownHolder.appendChild(profDropdown);

      // 4. Role & Mode switcher
      var isPart = !!p.csqtt_link_mode;
      roleSeg.setSelected(isPart ? "participant" : "owner");
      participantLogsRow.style.display = isPart ? "flex" : "none";
      participantLogsRow.setChecked(!!p.participant_show_logs);
      manualPortsRow.style.display = isPart ? "none" : "flex";
      manualPortsRow.setChecked(!!p.manual_ports_enabled);
      turnSubnetsBtn.style.display = isPart ? "none" : "";
      authSectionWrap.style.display = isPart ? "none" : "flex";

      // 5. Fingerprint
      fingerprintDropdown.setSelectedKey(p.selected_fingerprint || "chrome");

      // 6. Top Config Container (Participant vs Owner)
      var parsedLink = parseCsqttLink(p.csqtt_link);
      var hasLinkHashes = isPart && parsedLink && parsedLink.hashes && parsedLink.hashes.length > 0;

      if (isPart) {
        participantBox.style.display = "flex";
        ownerBox.style.display = "none";

        if (document.activeElement !== linkInput.input) {
          linkInput.setValue(p.csqtt_link || "");
        }

        if (hasLinkHashes) {
          linkInfoBadge.textContent = "VK хеши из ссылки: " + parsedLink.hashes.length;
        } else if (parsedLink && parsedLink.vkAccessToken) {
          linkInfoBadge.textContent = "VK токен из ссылки ✓";
        } else {
          linkInfoBadge.textContent = "";
        }
      } else {
        participantBox.style.display = "none";
        ownerBox.style.display = "flex";

        if (document.activeElement !== peerField.input) {
          peerField.setValue(p.peer || "");
        }

        if (p.manual_ports_enabled) {
          peerPortField.el.style.display = "flex";
          if (document.activeElement !== peerPortField.input) {
            peerPortField.setValue(String(p.server_peer_port || DEFAULT_SERVER_PEER_PORT));
          }
        } else {
          peerPortField.el.style.display = "none";
        }
      }

      // 7. Hash Mode controls state
      if (hasLinkHashes) {
        hashesActionWrap.style.display = "none";
        hashModeDropdown.style.display = "none";
      } else {
        hashesActionWrap.style.display = "flex";
        hashModeDropdown.style.display = "";

        var hashMode = p.vk_hash_mode || "manual";
        hashModeDropdown.setSelectedKey(hashMode);

        var hasVkToken = !!(p.vk_access_token_user_id || (p.vk_hashes && p.vk_hashes.indexOf("token_") !== -1));

        if (hashMode === "manual") {
          openHashesBtn.style.display = "flex";
          activeSessionBtn.style.display = "none";
          vkLoginBtn.style.display = "none";
        } else {
          openHashesBtn.style.display = "none";
          if (hasVkToken) {
            activeSessionBtn.style.display = "flex";
            vkLoginBtn.style.display = "none";
          } else {
            activeSessionBtn.style.display = "none";
            vkLoginBtn.style.display = "flex";
          }
        }
      }

      // 8. Work Mode
      var authMode = p.vk_auth_mode || "vkcalls";
      workModeDropdown.setSelectedKey(authMode);

      // 9. Captcha section visibility
      if (authMode === "legacy") {
        captchaSection.style.display = "flex";
        var isAutoCaptcha = (p.captcha_mode === "auto");
        autoCaptchaRow.setChecked(isAutoCaptcha);
        captchaManualOptions.style.display = isAutoCaptcha ? "none" : "flex";

        var isWv = (p.captcha_mode === "wv");
        wbvChip.classList.toggle("selected", isWv);
        rjsChip.classList.toggle("selected", !isWv);

        var isManualSolve = (p.captcha_wbv_solve_method === "manual" || p.captcha_solve_method === "manual");
        solveManualChip.classList.toggle("selected", isManualSolve);
        solveAutoChip.classList.toggle("selected", !isManualSolve);
      } else {
        captchaSection.style.display = "none";
      }

      // 10. SRTP & Transport
      srtpModeDropdown.setSelectedKey(p.srtp_mode || "auto");
      turnTransportDropdown.setSelectedKey(p.turn_transport || "udp");

      // 11. Workers Slider
      var rawWorkers = p.workers_per_hash !== undefined ? p.workers_per_hash : MIN_WORKERS;
      var wCount = roundWorkersToStep(rawWorkers);
      workersSlider.setValue(wCount);
      var sliderInput = workersSlider.querySelector(".m3-slider-input");
      if (sliderInput) sliderInput.disabled = isTunnelRunning;

      // 12. SNI
      if (document.activeElement !== sniField.input) {
        sniField.setValue(p.sni || "");
      }
    }

    // Subscribe to state updates
    window.App.onState(syncUI);
  }

  // ---------------------------------------------------------------------------
  // Register Tunnel / Settings Screen with window.App Navigation
  // ---------------------------------------------------------------------------
  window.App.registerScreen({
    id: "tunnel",
    title: "Туннель",
    icon: "VpnKey",
    order: 1,
    render: renderSettingsScreen
  });

  // ---------------------------------------------------------------------------
  // Public API
  // ---------------------------------------------------------------------------
  window.SettingsScreen = {
    render: renderSettingsScreen,
    showHashesDialog: showHashesDialog,
    showSecretsDialog: showSecretsDialog,
    showVkTokenRevokeDialog: showVkTokenRevokeDialog,
    showTurnSubnetsDialog: showTurnSubnetsDialog,
    showWorkersInfoDialog: showWorkersInfoDialog,
    showHashModeInfoDialog: showHashModeInfoDialog,
    showWorkModeInfoDialog: showWorkModeInfoDialog,
    showTurnTransportInfoDialog: showTurnTransportInfoDialog,
    showAutoVkRiskDialog: showAutoVkRiskDialog,
    showTcpTransportRiskDialog: showTcpTransportRiskDialog,
    showEditProfileNameDialog: showEditProfileNameDialog,
    showWifiAutoPauseInfoDialog: showWifiAutoPauseInfoDialog,
    stripVkUrl: stripVkUrl,
    parseCsqttLink: parseCsqttLink,
    normalizeHashes: normalizeHashes,
    applyAppTheme: applyAppTheme
  };

  window.TunnelScreen = window.SettingsScreen;
})();
