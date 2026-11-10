// Mock backend for browser development and load testing.
// Implements window.csqttMock = { call(method, params, emit, id), start(emit) }.

(function () {
  "use strict";

  function createDefaultProfile(name) {
    return {
      profile_name: name || "",
      peer: "",
      vk_hashes: "",
      secondary_vk_hash: "",
      workers_per_hash: 18,
      protocol: "udp",
      listen_port: 443,
      manual_ports_enabled: false,
      server_peer_port: 443,
      sni: "",
      no_dns: false,
      custom_turn_ips: "",
      selected_turn_subnet: "",
      turn_subnet_ip_limit: "ALL",
      user_agent: "",
      deploy_ip: "",
      deploy_login: "",
      deploy_ssh_port: "",
      ssh_keys_mode: false,
      docker_install: false,
      deploy_secret_path_enabled: true,
      deploy_secret_path_uuid: "",
      excluded_apps: "",
      whitelist_apps: "",
      detailed_logs: false,
      deploy_web_login: "",
      server_web_port: 8443,
      proxy_mode: "",
      proxy_host: "",
      proxy_port: 0,
      vk_auth_mode: "calls",
      obfs_mode: "unified",
      turn_transport: "udp",
      srtp_mode: "wrap",
      captcha_mode: "auto",
      captcha_solve_method: "auto",
      captcha_wbv_solve_method: "auto",
      vk_hash_mode: "manual",
      vk_access_token_user_id: "",
      is_whitelist: false,
      extra_workers: false,
      domain_routing_enabled: false,
      domain_is_whitelist: false,
      exclude_ru_zones_enabled: false,
      template_domains_enabled: false,
      direct_domains: "",
      direct_domains_black: "",
      direct_domains_white: "",
      ip_routing_enabled: false,
      ip_is_whitelist: false,
      direct_ips_black: "",
      direct_ips_white: "",
      selected_fingerprint: "chrome",
      active_client_ids: "3111527,3682744,2685278,2274003",
      vk_hash_check_results: "{}",
      csqtt_link: "",
      csqtt_link_mode: false,
      participant_show_logs: false,
      connection_generation: 0
    };
  }

  var mockSettings = {
    active_profile: 0,
    show_system_apps: false,
    logging_enabled: true,
    info_actions_expanded: false,
    info_project_expanded: false,
    auto_js_risk_acknowledged: false,
    tcp_transport_risk_acknowledged: false,
    battery_optimization_prompt_handled: false,
    auto_pause_on_wifi: false,
    theme_mode: "system",
    is_dynamic_color: false,
    theme_palette: "indigo",
    update_last_check_at: Date.now() - 3600000,
    update_latest_version: "2.5.0",
    update_last_error: "",
    update_check_interval_hours: 24,
    update_postpone_until: 0,
    update_postpone_version: "",
    update_dialog_last_shown_version: "",
    update_dialog_last_shown_at: 0,
    update_dialog_last_action_version: "",
    update_dialog_last_action: "",
    update_dialog_last_action_at: 0,
    client_id_check_results: "{}",
    tunnel_last_diagnostic: "",
    tunnel_last_diagnostic_at: 0,
    tunnel_was_running: false,
    split_tunnel_whitelist_migrated: false,
    profiles: [
      createDefaultProfile("Профиль 0"),
      createDefaultProfile("Профиль 1"),
      createDefaultProfile("Профиль 2"),
      createDefaultProfile("Профиль 3")
    ]
  };

  var mockSecrets = {
    connection_password_encrypted: "",
    vk_access_token_encrypted: ""
  };

  var mockStatus = {
    state: "stopped",
    message: "",
    client_ip: "",
    dns: "",
    active_workers: 0,
    bytes_up: 0,
    bytes_down: 0
  };

  var mockLogs = [
    "[NET] Служба csqtt-win запущена (режим Mock 0.1.0)",
    "[OK] Конфигурация проверена: профиль 0 готов",
    "[OK] Инициализирован стек протокола WRAP-SRTP"
  ];

  var emitFn = null;
  var floodTimer = null;
  var statsTimer = null;
  var logCounter = 1;

  var SAMPLE_LOGS = [
    "[OK] Сессия установлена с TURN сервером 185.125.218.44:3478",
    "[NET] Получен ответ от узла: задержка 18.4 мс",
    "[ВОРКЕР] Воркер #1 подключен к VK calls каналу",
    "[ВОРКЕР] Воркер #2 подключен к VK calls каналу",
    "[WRAP] Пакет SRTP синхронизирован (seq=10482)",
    "[СЕТЬ] Туннель маршрутизирует трафик (потоков: 18)",
    "[OK] Проверка хеша VK завершена успешно",
    "[ERR] Ошибка резолвинга DNS для хоста fallback",
    "[КАПЧА] Капча не требуется, сессия валидна",
    "[ОШИБКА] Тайм-аут сокета воркера #7, переподключение..."
  ];

  function emitStatus() {
    if (emitFn) emitFn("status", { status: JSON.parse(JSON.stringify(mockStatus)) });
  }

  function emitLog(line) {
    mockLogs.push(line);
    if (mockLogs.length > 5000) mockLogs.shift();
    if (emitFn) emitFn("log", { line: line });
  }

  function start(emit) {
    emitFn = emit;
    emit("service", { running: true });
    emitStatus();

    // Periodic occasional background logs
    setInterval(function () {
      if (mockStatus.state === "connected") {
        var sample = SAMPLE_LOGS[Math.floor(Math.random() * SAMPLE_LOGS.length)];
        emitLog(sample + " [" + (++logCounter) + "]");
      }
    }, 2400);

    // Connected stats simulator
    statsTimer = setInterval(function () {
      if (mockStatus.state === "connected") {
        mockStatus.bytes_up += Math.floor(Math.random() * 45000) + 5000;
        mockStatus.bytes_down += Math.floor(Math.random() * 180000) + 20000;
        emitStatus();
      }
    }, 1000);
  }

  function call(method, params, emit, id) {
    emitFn = emit;
    return new Promise(function (resolve, reject) {
      if (method === "version") {
        resolve({ version: "0.1.0-mock" });
        return;
      }

      if (method === "open_url") {
        window.open(params.url, "_blank");
        resolve(null);
        return;
      }

      if (method === "service_setup") {
        setTimeout(function () { resolve(null); }, 1500);
        return;
      }

      if (method === "proc_start") {
        var procId = id || 999;
        var args = params.args || [];
        var subcmd = args[0] || "test";
        setTimeout(function () {
          if (emit) emit("proc_output", { proc: procId, text: "csqtt " + subcmd + " started...\n" });
        }, 100);
        setTimeout(function () {
          if (emit) emit("proc_output", { proc: procId, text: "csqtt " + subcmd + " finished successfully.\n" });
          resolve({ code: 0 });
        }, 800);
        return;
      }

      if (method === "proc_input" || method === "proc_kill") {
        resolve(null);
        return;
      }

      if (method === "ipc") {
        var req = params.request || {};
        var cmd = req.cmd;

        if (cmd === "status") {
          resolve({ type: "status", status: JSON.parse(JSON.stringify(mockStatus)) });
          return;
        }

        if (cmd === "connect") {
          mockStatus.state = "starting";
          mockStatus.message = "Запуск соединения...";
          emitStatus();
          emitLog("[OK] Запуск VPN туннеля...");

          setTimeout(function () {
            mockStatus.state = "connected";
            mockStatus.message = "";
            mockStatus.client_ip = "10.8.0.2";
            mockStatus.dns = "1.1.1.1, 8.8.8.8";
            mockStatus.active_workers = 18;
            emitStatus();
            emitLog("[OK] VPN туннель успешно подключен: IP 10.8.0.2, DNS 1.1.1.1");
            resolve({ type: "status", status: JSON.parse(JSON.stringify(mockStatus)) });
          }, 900);
          return;
        }

        if (cmd === "disconnect") {
          mockStatus.state = "stopped";
          mockStatus.message = "";
          mockStatus.client_ip = "";
          mockStatus.active_workers = 0;
          emitStatus();
          emitLog("[NET] Туннель остановлен пользователем");
          resolve({ type: "status", status: JSON.parse(JSON.stringify(mockStatus)) });
          return;
        }

        if (cmd === "get_settings") {
          resolve({ type: "settings", settings: JSON.parse(JSON.stringify(mockSettings)) });
          return;
        }

        if (cmd === "set_settings") {
          mockSettings = JSON.parse(JSON.stringify(req.settings));
          resolve({ type: "ok" });
          return;
        }

        if (cmd === "set_secret") {
          mockSecrets[req.key] = req.value;
          emitLog("[OK] Секрет " + req.key + " обновлен");
          resolve({ type: "ok" });
          return;
        }

        if (cmd === "import") {
          emitLog("[OK] Конфигурация успешно импортирована");
          resolve({ type: "ok" });
          return;
        }

        if (cmd === "export") {
          resolve({ type: "exported", json: JSON.stringify(mockSettings, null, 2) });
          return;
        }

        if (cmd === "logs") {
          var count = req.tail || 500;
          resolve({ type: "logs", lines: mockLogs.slice(-count) });
          return;
        }

        if (cmd === "logs_clear") {
          mockLogs = [];
          resolve({ type: "ok" });
          return;
        }

        resolve({ type: "ok" });
        return;
      }

      reject(new Error("Неизвестный mock метод: " + method));
    });
  }

  function flood(linesPerSecond) {
    if (floodTimer) {
      clearInterval(floodTimer);
      floodTimer = null;
    }
    if (!linesPerSecond || linesPerSecond <= 0) return;

    var intervalMs = Math.max(5, Math.floor(1000 / linesPerSecond));
    var batchSize = Math.max(1, Math.floor(linesPerSecond / (1000 / intervalMs)));

    floodTimer = setInterval(function () {
      for (var i = 0; i < batchSize; i++) {
        var sample = SAMPLE_LOGS[Math.floor(Math.random() * SAMPLE_LOGS.length)];
        emitLog(sample + " [flood #" + (++logCounter) + "]");
      }
    }, intervalMs);
  }

  window.csqttMock = {
    call: call,
    start: start,
    flood: flood,
    getSettings: function () { return mockSettings; },
    getStatus: function () { return mockStatus; }
  };
})();
