/**
 * Connection Screen for csqtt-win
 *
 * Implements the connection tab, big toggle button, tunnel status display,
 * server configuration fields, dropdowns, slider, and dialogs matching
 * ConnectionTab.kt and ui/tunnel/*.
 */

(function () {
  "use strict";

  var SECRET_CONNECTION_PASSWORD = "connection_password_encrypted";
  var SECRET_VK_TOKEN = "vk_access_token_encrypted";

  var MIN_WORKERS = 10;
  var MAX_WORKERS = 100;
  var WORKERS_STEP = 10;

  function roundWorkersToStep(val) {
    var num = Number(val);
    if (isNaN(num)) num = MIN_WORKERS;
    var rounded = Math.round(num / WORKERS_STEP) * WORKERS_STEP;
    if (rounded < MIN_WORKERS) rounded = MIN_WORKERS;
    if (rounded > MAX_WORKERS) rounded = MAX_WORKERS;
    return rounded;
  }

  function formatBytes(bytes) {
    if (!bytes || bytes <= 0) return "0 Б";
    var units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
    var i = Math.floor(Math.log(bytes) / Math.log(1024));
    if (i >= units.length) i = units.length - 1;
    var val = bytes / Math.pow(1024, i);
    return (i === 0 ? val.toFixed(0) : val.toFixed(1)) + " " + units[i];
  }

  function formatUptime(seconds) {
    var total = Math.max(0, seconds || 0);
    var h = Math.floor(total / 3600);
    var m = Math.floor((total % 3600) / 60);
    var s = total % 60;
    var hh = h < 10 ? "0" + h : String(h);
    var mm = m < 10 ? "0" + m : String(m);
    var ss = s < 10 ? "0" + s : String(s);
    return hh + ":" + mm + ":" + ss;
  }

  function parseLink(raw) {
    if (!raw || !raw.startsWith("csqtt://")) return null;
    try {
      var queryIdx = raw.indexOf("?");
      if (queryIdx === -1) return null;
      var params = new URLSearchParams(raw.slice(queryIdx + 1));
      return {
        host: params.get("host") || "",
        peer: params.get("peer") || "",
        password: params.get("password") || "",
        hashes: (params.get("hashes") || "").split(",").filter(function (h) { return h.length >= 16; }),
        vkAccessToken: params.get("vk_access_token") || ""
      };
    } catch (_) {
      return null;
    }
  }

  function renderConnectionScreen(container) {
    container.innerHTML = "";

    var uptimeSeconds = 0;
    var uptimeTimer = null;

    // Hero Section Card: Connect Button & State
    var heroCard = window.UI.sectionCard({ className: "connection-hero" });

    var connectBtn = window.UI.h("button", {
      className: "connect-btn inactive",
      attrs: { type: "button", title: "Включить VPN" }
    });
    connectBtn.appendChild(window.Icons.el("ConnectLogo", 172));
    heroCard.appendChild(connectBtn);

    var statusText = window.UI.h("div", {
      className: "connect-status-text",
      text: "Отключено"
    });
    heroCard.appendChild(statusText);

    var uptimeText = window.UI.h("div", {
      className: "connect-uptime-text",
      text: "00:00:00"
    });
    heroCard.appendChild(uptimeText);

    // Status statistics grid
    var statsGrid = window.UI.h("div", { className: "status-grid" });

    function createStatCard(label, initialVal) {
      var card = window.UI.h("div", { className: "status-stat-card" });
      var lbl = window.UI.h("span", { className: "status-stat-label", text: label });
      var val = window.UI.h("span", { className: "status-stat-value", text: initialVal || "—" });
      card.appendChild(lbl);
      card.appendChild(val);
      card.setValue = function (v) { val.textContent = v || "—"; };
      return card;
    }

    var statIp = createStatCard("IP адрес клиента", "");
    var statDns = createStatCard("DNS серверы", "");
    var statWorkers = createStatCard("Активные воркеры", "");
    var statTraffic = createStatCard("Трафик (↓ / ↑)", "");

    statsGrid.appendChild(statIp);
    statsGrid.appendChild(statDns);
    statsGrid.appendChild(statWorkers);
    statsGrid.appendChild(statTraffic);
    heroCard.appendChild(statsGrid);
    container.appendChild(heroCard);

    // Configuration Card
    var configCard = window.UI.sectionCard();
    var configTitle = window.UI.h("div", {
      className: "title-medium",
      text: "Параметры подключения"
    });
    configCard.appendChild(configTitle);

    var configContent = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "14px" }
    });
    configCard.appendChild(configContent);
    container.appendChild(configCard);

    function updateStatusUI(st) {
      st = st || {};
      var s = st.state || "stopped";
      var isConnected = s === "connected";
      var isStarting = s === "starting";
      var isPaused = s === "paused";
      var isReconnecting = s === "reconnecting";
      var isFailed = s === "failed";

      connectBtn.className = "connect-btn" +
        (isConnected ? " active" : (isStarting ? " starting" : " inactive"));

      if (isConnected) {
        statusText.textContent = "Подключено";
        statusText.style.color = "var(--md-sys-color-primary)";
        if (!uptimeTimer) {
          uptimeTimer = setInterval(function () {
            uptimeSeconds++;
            uptimeText.textContent = formatUptime(uptimeSeconds);
          }, 1000);
        }
      } else if (isStarting) {
        statusText.textContent = "Подключение";
        statusText.style.color = "var(--md-sys-color-primary)";
      } else if (isPaused) {
        statusText.textContent = "Ожидание. Автопауза при Wi-Fi";
        statusText.style.color = "var(--md-sys-color-tertiary)";
      } else if (isReconnecting) {
        statusText.textContent = "Переподключение: " + (st.message || "...");
        statusText.style.color = "var(--md-sys-color-tertiary)";
      } else if (isFailed) {
        statusText.textContent = "Ошибка: " + (st.message || "сбой подключения");
        statusText.style.color = "var(--md-sys-color-error)";
      } else {
        statusText.textContent = "Отключено";
        statusText.style.color = "var(--md-sys-color-on-surface-variant)";
        if (uptimeTimer) {
          clearInterval(uptimeTimer);
          uptimeTimer = null;
        }
        uptimeSeconds = 0;
        uptimeText.textContent = "00:00:00";
      }

      statIp.setValue(st.client_ip || "");
      statDns.setValue(st.dns || "");
      statWorkers.setValue(st.active_workers ? String(st.active_workers) : "0");
      var downStr = formatBytes(st.bytes_down || 0);
      var upStr = formatBytes(st.bytes_up || 0);
      statTraffic.setValue(downStr + " / " + upStr);
    }

    // Toggle button handler
    connectBtn.addEventListener("click", function () {
      var currentSt = (window.App.state.status && window.App.state.status.state) || "stopped";
      if (currentSt === "connected" || currentSt === "starting") {
        window.csqtt.disconnect().catch(function (err) {
          window.UI.toast("Ошибка отключения: " + err.message);
        });
        return;
      }

      // Validate configuration before connect
      var prof = window.App.profile();
      if (!prof) {
        window.UI.toast("Настройки не загружены");
        return;
      }

      if (prof.csqtt_link_mode) {
        var parsed = parseLink(prof.csqtt_link);
        if (!parsed) {
          window.UI.toast("Укажите корректную ссылку csqtt://");
          return;
        }
      } else {
        var p = (prof.peer || "").trim();
        if (!p || p === "0.0.0.0" || p === "127.0.0.1" || p === "localhost" || p.indexOf(":") !== -1) {
          window.UI.toast("Укажите корректный IP сервера или домен");
          return;
        }
      }

      window.csqtt.connect().catch(function (err) {
        window.UI.toast("Ошибка запуска: " + err.message);
      });
    });

    // Render configuration inputs
    function renderConfigInputs(appState) {
      configContent.innerHTML = "";
      var prof = window.App.profile();
      if (!prof) return;

      var isLinkMode = !!prof.csqtt_link_mode;

      if (isLinkMode) {
        // Participant mode: csqtt_link field
        var linkField = window.UI.textField({
          label: "Ссылка csqtt://",
          placeholder: "csqtt://connect?v=2&host=ip&peer=порт&password=пароль",
          value: prof.csqtt_link || "",
          onInput: function (val) {
            prof.csqtt_link = val.trim();
          },
          onChange: function (val) {
            window.App.saveSettings(function (s) {
              var p = s.profiles[s.active_profile || 0];
              if (p) p.csqtt_link = val.trim();
            });
          }
        });
        configContent.appendChild(linkField.el);

        var parsedLink = parseLink(prof.csqtt_link);
        if (parsedLink && parsedLink.hashes.length > 0) {
          var hashesInfo = window.UI.h("div", {
            className: "label-large text-primary",
            text: "VK хеши из ссылки: " + parsedLink.hashes.length
          });
          configContent.appendChild(hashesInfo);
        }
      } else {
        // Owner mode: Peer and optional server_peer_port
        if (prof.manual_ports_enabled) {
          var peerRow = window.UI.h("div", {
            style: { display: "flex", gap: "10px", width: "100%" }
          });

          var peerField = window.UI.textField({
            label: "IP сервера или домен",
            placeholder: "1.2.3.4",
            value: prof.peer || "",
            onChange: function (val) {
              window.App.saveSettings(function (s) {
                var p = s.profiles[s.active_profile || 0];
                if (p) p.peer = val.trim();
              });
            }
          });
          peerField.el.style.flex = "2";
          peerRow.appendChild(peerField.el);

          var portField = window.UI.textField({
            label: "Порт PEER",
            placeholder: "443",
            value: String(prof.server_peer_port || 443),
            onChange: function (val) {
              var portNum = parseInt(val, 10);
              if (portNum >= 1 && portNum <= 65535) {
                window.App.saveSettings(function (s) {
                  var p = s.profiles[s.active_profile || 0];
                  if (p) p.server_peer_port = portNum;
                });
              }
            }
          });
          portField.el.style.flex = "1";
          peerRow.appendChild(portField.el);
          configContent.appendChild(peerRow);
        } else {
          var singlePeer = window.UI.textField({
            label: "IP сервера или домен (без порта)",
            placeholder: "1.2.3.4 (или test.com)",
            value: prof.peer || "",
            onChange: function (val) {
              window.App.saveSettings(function (s) {
                var p = s.profiles[s.active_profile || 0];
                if (p) p.peer = val.trim();
              });
            }
          });
          configContent.appendChild(singlePeer.el);
        }

        // VK Hashes or Auth button
        var isManualHashes = prof.vk_hash_mode === "manual";
        if (isManualHashes) {
          var hashesBtn = window.UI.button({
            text: "Ссылки VK звонков",
            icon: "Phone",
            variant: "outlined",
            fullWidth: true,
            onClick: showHashesDialog
          });
          configContent.appendChild(hashesBtn);
        } else {
          var vkAuthBtn = window.UI.button({
            text: "Вход VK (токен авторизации)",
            icon: "Login",
            variant: "outlined",
            fullWidth: true,
            onClick: showVkTokenDialog
          });
          configContent.appendChild(vkAuthBtn);
        }

        // Dropdown: VK hash mode
        var hashModeDropdown = window.UI.dropdown({
          title: "Режим хешей",
          selectedKey: prof.vk_hash_mode || "manual",
          options: [
            { key: "manual", label: "Ручной" },
            { key: "auto_api", label: "Авто API" },
            { key: "auto_js", label: "Авто ВК" }
          ],
          indicatorProvider: function (m) {
            if (m === "manual") return { progress: 0.78, color: "#43A047" };
            return { progress: 1.0, color: "#1475FD" };
          },
          onTitleInfo: function () {
            window.UI.infoDialog("Режим хешей", "Определяет способ получения ссылок звонков VK (вручную или автоматически через токен).");
          },
          onSelected: function (mode) {
            window.App.saveSettings(function (s) {
              var p = s.profiles[s.active_profile || 0];
              if (p) p.vk_hash_mode = mode;
            });
          }
        });
        configContent.appendChild(hashModeDropdown);

        // Authorization button (tunnel password)
        var authBtn = window.UI.button({
          text: "Авторизация (пароль)",
          icon: "Key",
          variant: "outlined",
          fullWidth: true,
          onClick: showSecretsDialog
        });
        configContent.appendChild(authBtn);
      }

      // Dropdown: VK auth mode (calls, captcha, auto_js)
      var authModeDropdown = window.UI.dropdown({
        title: "Режим кредов",
        selectedKey: prof.vk_auth_mode || "calls",
        options: [
          { key: "calls", label: "Авто" },
          { key: "captcha", label: "Капча" },
          { key: "auto_js", label: "Авто ВК" }
        ],
        indicatorProvider: function (m) {
          if (m === "captcha") return { progress: 0.3, color: "#E53935" };
          if (m === "calls") return { progress: 0.78, color: "#43A047" };
          return { progress: 1.0, color: "#1475FD" };
        },
        onTitleInfo: function () {
          window.UI.infoDialog("Режим кредов", "Метод авторизации и поддержания сессии в звонках ВКонтакте.");
        },
        onSelected: function (mode) {
          window.App.saveSettings(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) p.vk_auth_mode = mode;
          });
        }
      });
      configContent.appendChild(authModeDropdown);

      // Dropdown: SRTP Mode
      var srtpDropdown = window.UI.dropdown({
        title: "Режим SRTP",
        selectedKey: prof.srtp_mode || "wrap",
        options: [
          { key: "wrap", label: "WRAP-SRTP" },
          { key: "dtls", label: "DTLS-SRTP" },
          { key: "auto", label: "Авто" }
        ],
        onSelected: function (mode) {
          window.App.saveSettings(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) p.srtp_mode = mode;
          });
        }
      });
      configContent.appendChild(srtpDropdown);

      // Dropdown: Turn transport
      var transportDropdown = window.UI.dropdown({
        title: "Транспорт",
        selectedKey: prof.turn_transport || "udp",
        options: [
          { key: "udp", label: "UDP" },
          { key: "tcp", label: "TCP" }
        ],
        onTitleInfo: function () {
          window.UI.infoDialog("Транспорт TURN", "UDP обеспечивает минимальную задержку. TCP используется при блокировках UDP трафика.");
        },
        onSelected: function (tr) {
          window.App.saveSettings(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) p.turn_transport = tr;
          });
        }
      });
      configContent.appendChild(transportDropdown);

      // Slider: Workers per hash
      var isTunnelRunning = !!(appState && appState.status && (appState.status.state === "connected" || appState.status.state === "starting"));
      var rawWorkers = prof.workers_per_hash !== undefined ? prof.workers_per_hash : MIN_WORKERS;
      var workersSlider = window.UI.slider({
        label: "Настройка потоков",
        min: MIN_WORKERS,
        max: MAX_WORKERS,
        step: WORKERS_STEP,
        value: roundWorkersToStep(rawWorkers),
        valueFormatter: function (v) { return v + " (" + (v * 2) + " Мбит/с)"; },
        enabled: !isTunnelRunning,
        onInfo: function () {
          showWorkersInfoDialog();
        },
        onValueChange: function (val) {
          var rounded = roundWorkersToStep(val);
          window.App.saveSettings(function (s) {
            var p = s.profiles[s.active_profile || 0];
            if (p) p.workers_per_hash = rounded;
          });
        }
      });
      configContent.appendChild(workersSlider);
    }

    function showWorkersInfoDialog() {
      var body = window.UI.h("div", { style: { display: "flex", flexDirection: "column", gap: "8px" } });
      var lines = [
        "• Каждый поток TURN имеет скорость около 2 Мбит/с.",
        "• Рекомендуется выбирать количество потоков исходя из реальной скорости вашего интернет-подключения:",
        "  — 10 потоков: для скорости 20 Мбит/с",
        "  — 20 потоков: для скорости 40 Мбит/с",
        "  — 30 потоков: для скорости 60 Мбит/с",
        "  — и далее по нарастающей."
      ];
      lines.forEach(function (l) {
        body.appendChild(window.UI.h("p", { className: "body-small text-muted", text: l }));
      });
      window.UI.infoDialog("Настройка потоков", body);
    }

    function showSecretsDialog() {
      var passField = window.UI.textField({
        label: "Пароль подключения",
        isPassword: true,
        placeholder: "Введите пароль сервера",
        value: ""
      });

      window.UI.dialog({
        title: "Авторизация",
        content: passField.el,
        actions: [
          { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
          {
            text: "Сохранить",
            variant: "filled",
            onClick: function (d) {
              var pw = passField.getValue().trim();
              window.csqtt.setSecret(SECRET_CONNECTION_PASSWORD, pw).then(function () {
                window.UI.toast("Пароль сохранён");
                d.close();
              }).catch(function (err) {
                window.UI.toast("Ошибка: " + err.message);
              });
            }
          }
        ]
      });
    }

    function showVkTokenDialog() {
      var tokenField = window.UI.textField({
        label: "VK Access Token",
        placeholder: "vk1.a...",
        value: ""
      });

      window.UI.dialog({
        title: "VK Токен",
        content: tokenField.el,
        actions: [
          { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
          {
            text: "Сохранить",
            variant: "filled",
            onClick: function (d) {
              var tok = tokenField.getValue().trim();
              window.csqtt.setSecret(SECRET_VK_TOKEN, tok).then(function () {
                window.UI.toast("VK токен сохранён");
                d.close();
              }).catch(function (err) {
                window.UI.toast("Ошибка: " + err.message);
              });
            }
          }
        ]
      });
    }

    function showHashesDialog() {
      var prof = window.App.profile() || {};
      var currentHashes = (prof.vk_hashes || "").split(",").map(function (s) { return s.trim(); });

      var wrap = window.UI.h("div", { style: { display: "flex", flexDirection: "column", gap: "8px" } });
      var fields = [];
      for (var i = 0; i < 6; i++) {
        var f = window.UI.textField({
          label: "Хеш звонка " + (i + 1),
          placeholder: "Вставьте хеш или ссылку VK",
          value: currentHashes[i] || ""
        });
        fields.push(f);
        wrap.appendChild(f.el);
      }

      window.UI.dialog({
        title: "Ссылки VK звонков",
        content: wrap,
        actions: [
          { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
          {
            text: "Сохранить",
            variant: "filled",
            onClick: function (d) {
              var collected = fields.map(function (f) {
                return f.getValue().trim();
              }).filter(function (h) {
                return h.length >= 16;
              });

              window.App.saveSettings(function (s) {
                var p = s.profiles[s.active_profile || 0];
                if (p) p.vk_hashes = collected.join(",");
              }).then(function () {
                window.UI.toast("Хеши сохранены (" + collected.length + ")");
                d.close();
              });
            }
          }
        ]
      });
    }

    // Subscribe to state updates
    window.App.onState(function (st) {
      updateStatusUI(st.status);
      renderConfigInputs(st);
    });
  }

  // Register Connection Screen
  window.App.registerScreen({
    id: "connection",
    title: "Подкл.",
    icon: "PowerSettingsNew",
    order: 0,
    render: renderConnectionScreen
  });
})();
