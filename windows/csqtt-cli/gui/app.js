/**
 * Application Core and Navigation for csqtt-win
 *
 * PUBLIC API:
 *
 * App.registerScreen({ id, title, icon, order, render(container), onShow(), onHide() })
 *   Registers a tab screen. Tabs appear in the bottom navigation bar sorted by `order`.
 *   - id: string unique screen identifier (e.g. "connection", "tunnel", "logs")
 *   - title: string display label for navigation tab
 *   - icon: string icon name from window.Icons
 *   - order: number sorting order (0=connection, 1=tunnel, 2=deploy, 3=logs, 4=exceptions)
 *   - render: function(container: HTMLElement) called once to render DOM content
 *   - onShow: optional function() called when screen becomes visible
 *   - onHide: optional function() called when screen is hidden
 *
 * App.showScreen(id)
 *   Switches the active screen to `id`.
 *
 * App.state: { settings: object|null, status: object|null, serviceRunning: boolean, captchaActive: boolean }
 *   The shared application state store.
 *
 * App.onState(fn) -> unsubscribeFunction
 *   Subscribes `fn(state)` to be called whenever application state changes.
 *   Immediately calls `fn(App.state)`.
 *
 * App.refreshSettings() -> Promise<settings>
 *   Loads latest settings from the service and updates state.
 *
 * App.saveSettings(mutatorFn) -> Promise<settings>
 *   Calls csqtt.updateSettings(mutatorFn). Shows UI toast on error.
 *
 * App.profile() -> object|null
 *   Returns the active Profile object inside settings (settings.profiles[settings.active_profile]).
 *
 * App.showQuickSettingsDialog()
 *   Opens the quick settings modal dialog mirroring Android FloatingToolbar.
 */

(function () {
  "use strict";

  var screens = {};
  var screenOrder = [];
  var currentScreenId = null;
  var stateListeners = [];

  var state = {
    settings: null,
    status: {
      state: "stopped",
      message: "",
      client_ip: "",
      dns: "",
      active_workers: 0,
      bytes_up: 0,
      bytes_down: 0
    },
    serviceRunning: true,
    captchaActive: false,
    unreadErrors: 0
  };

  function notifyState() {
    stateListeners.slice().forEach(function (fn) {
      try {
        fn(state);
      } catch (err) {
        console.error("App state listener error", err);
      }
    });
  }

  function onState(fn) {
    stateListeners.push(fn);
    try {
      fn(state);
    } catch (err) {
      console.error("App state listener initial call error", err);
    }
    return function () {
      var idx = stateListeners.indexOf(fn);
      if (idx >= 0) stateListeners.splice(idx, 1);
    };
  }

  function profile() {
    if (!state.settings || !Array.isArray(state.settings.profiles)) return null;
    var idx = state.settings.active_profile || 0;
    if (idx < 0 || idx >= state.settings.profiles.length) idx = 0;
    return state.settings.profiles[idx];
  }

  function applyTheme(themeMode) {
    var mode = themeMode || (state.settings && state.settings.theme_mode) || "system";
    document.documentElement.setAttribute("data-theme", mode);
  }

  function refreshSettings() {
    return window.csqtt.getSettings().then(function (s) {
      state.settings = s;
      applyTheme(s.theme_mode);
      notifyState();
      return s;
    }).catch(function (err) {
      console.warn("Could not load settings:", err);
    });
  }

  function saveSettings(mutator) {
    return window.csqtt.updateSettings(mutator).then(function (s) {
      state.settings = s;
      applyTheme(s.theme_mode);
      notifyState();
      return s;
    }).catch(function (err) {
      window.UI.toast("Ошибка сохранения: " + (err.message || err));
      throw err;
    });
  }

  function registerScreen(def) {
    screens[def.id] = def;
    if (screenOrder.indexOf(def.id) === -1) {
      screenOrder.push(def.id);
    }
    screenOrder.sort(function (a, b) {
      var oa = screens[a].order !== undefined ? screens[a].order : 99;
      var ob = screens[b].order !== undefined ? screens[b].order : 99;
      return oa - ob;
    });

    renderNavBar();

    // If this is the first screen or "connection", show it
    if (!currentScreenId || def.id === "connection") {
      showScreen(def.id);
    }
  }

  function getVisibleScreenOrder() {
    var p = profile();
    var linkMode = !!(p && p.csqtt_link_mode);
    var showLogs = !!(p && p.participant_show_logs);
    return screenOrder.filter(function (id) {
      if (linkMode) {
        if (id === "deploy") return false;
        if (id === "logs" && !showLogs) return false;
      }
      return true;
    });
  }

  // Snapshot of the last visible screen list used to build nav buttons,
  // so we can skip full DOM reconstruction when only indicators change.
  var _navLastVisible = null;

  function renderNavBar() {
    var nav = document.getElementById("nav-bar");
    if (!nav) return;

    var visible = getVisibleScreenOrder();

    if (visible.length <= 1) {
      nav.classList.add("hidden");
      _navLastVisible = null;
      nav.innerHTML = "";
      return;
    }
    nav.classList.remove("hidden");

    // Rebuild buttons only when the set/order/labels/icons of visible screens change.
    var visibleKey = visible.map(function (id) {
      var scr = screens[id];
      return id + ":" + (scr ? (scr.title || id) + ":" + (scr.icon || "Info") : "");
    }).join("|");

    if (_navLastVisible !== visibleKey) {
      _navLastVisible = visibleKey;
      nav.innerHTML = "";

      var track = window.UI.h("div", { className: "nav-track" });
      var indicator = window.UI.h("div", { className: "nav-indicator-pill" });
      track.appendChild(indicator);

      var count = visible.length;
      var itemWidthPercent = 100 / count;
      indicator.style.width = itemWidthPercent + "%";

      visible.forEach(function (id) {
        var scr = screens[id];
        if (!scr) return;

        var item = window.UI.h("button", {
          className: "nav-item",
          attrs: { type: "button", "data-screen-id": id },
          on: {
            click: function () {
              showScreen(id);
            }
          }
        });

        var iconWrap = window.UI.h("div", { className: "nav-item-icon-wrap" });
        var iconName = scr.icon || "Info";
        var iconEl = window.Icons.el(iconName, 22);
        iconWrap.appendChild(iconEl);

        // Badge placeholder – updated in-place by updateNavBadges()
        var badge = window.UI.h("span", { className: "nav-badge", attrs: { "data-badge": "1" } });
        badge.style.display = "none";
        iconWrap.appendChild(badge);

        item.appendChild(iconWrap);
        var label = window.UI.h("span", { className: "label-small", text: scr.title || id });
        item.appendChild(label);
        track.appendChild(item);
      });

      nav.appendChild(track);
    }

    updateNavIndicator();
    updateNavBadges();
  }

  function updateNavBadges() {
    var nav = document.getElementById("nav-bar");
    if (!nav) return;
    var items = nav.querySelectorAll(".nav-item");
    for (var i = 0; i < items.length; i++) {
      var id = items[i].getAttribute("data-screen-id");
      var badge = items[i].querySelector(".nav-badge");
      if (!badge) continue;
      if (id === "logs" && state.unreadErrors > 0 && currentScreenId !== "logs") {
        badge.textContent = String(state.unreadErrors);
        badge.style.display = "";
      } else {
        badge.style.display = "none";
      }
    }
  }

  function updateNavIndicator() {
    var nav = document.getElementById("nav-bar");
    if (!nav) return;
    var visible = getVisibleScreenOrder();
    var idx = visible.indexOf(currentScreenId);
    if (idx === -1) idx = 0;
    var indicator = nav.querySelector(".nav-indicator-pill");
    if (indicator) {
      var offsetPercent = idx * 100;
      indicator.style.transform = "translateX(" + offsetPercent + "%)";
    }

    var items = nav.querySelectorAll(".nav-item");
    for (var i = 0; i < items.length; i++) {
      if (items[i].getAttribute("data-screen-id") === currentScreenId) {
        items[i].classList.add("selected");
      } else {
        items[i].classList.remove("selected");
      }
    }
  }

  function showScreen(id) {
    if (!screens[id]) return;
    var visible = getVisibleScreenOrder();
    if (visible.indexOf(id) === -1) {
      id = "connection";
      if (!screens[id]) return;
    }
    if (currentScreenId === id) return;

    var prevScr = currentScreenId ? screens[currentScreenId] : null;
    if (prevScr && prevScr.onHide) {
      try { prevScr.onHide(); } catch (e) { console.error(e); }
    }

    var content = document.getElementById("app-content");
    if (!content) return;

    // Hide all existing screen containers
    var children = content.querySelectorAll(":scope > .screen-container");
    for (var i = 0; i < children.length; i++) {
      children[i].classList.add("hidden");
    }

    currentScreenId = id;
    if (id === "logs") {
      state.unreadErrors = 0;
    }

    var scr = screens[id];
    var container = content.querySelector(':scope > .screen-container[data-screen="' + id + '"]');
    if (!container) {
      container = window.UI.h("div", {
        className: "screen-container",
        attrs: { "data-screen": id }
      });
      content.appendChild(container);
      if (scr.render) {
        scr.render(container);
      }
    }
    container.classList.remove("hidden");

    updateNavIndicator();
    updateNavBadges();
    if (scr.onShow) {
      try { scr.onShow(); } catch (e) { console.error(e); }
    }
  }

  function updateNavigationState() {
    var visible = getVisibleScreenOrder();
    if (currentScreenId && visible.indexOf(currentScreenId) === -1) {
      showScreen("connection");
    }
    renderNavBar();
  }

  onState(function () {
    updateNavigationState();
  });

  function updateServiceBanner(running) {
    var banner = document.getElementById("service-banner");
    if (!banner) return;
    // The host repeats {running:false} every 2 s while the service is absent; rebuilding the
    // banner each time would reset the setup button in the middle of an installation.
    if (banner.dataset.running === String(!!running)) return;
    banner.dataset.running = String(!!running);
    if (running) {
      banner.classList.add("hidden");
      banner.innerHTML = "";
    } else {
      banner.classList.remove("hidden");
      banner.innerHTML = "";

      var hint = window.UI.h("span", {
        text: "Служба csqtt-win не запущена."
      });
      banner.appendChild(hint);

      var btn = window.UI.button({
        text: "Установить и запустить службу",
        variant: "filled",
        className: "compact",
        onClick: function () {
          btn.disabled = true;
          var labelSpan = btn.querySelector("span");
          if (labelSpan) {
            labelSpan.textContent = "Установка службы…";
          } else {
            btn.textContent = "Установка службы…";
          }

          window.csqtt.setupService().catch(function (err) {
            btn.disabled = false;
            if (labelSpan) {
              labelSpan.textContent = "Установить и запустить службу";
            } else {
              btn.textContent = "Установить и запустить службу";
            }
            var msg = (err && err.message) ? err.message : String(err);
            window.UI.toast(msg);
          });
        }
      });
      banner.appendChild(btn);
    }
  }

  function initHeader() {
    var logo = document.getElementById("brand-logo");
    if (logo && !logo.firstChild) {
      logo.appendChild(window.Icons.el("CsqttLogo", 22));
    }

    var actions = document.getElementById("header-actions");
    if (actions && !actions.firstChild) {
      // Support Developer
      var donateBtn = window.UI.iconButton({
        icon: "Heart",
        title: "Поддержать разработчика",
        size: 22,
        onClick: showSupportDeveloperDialog
      });
      actions.appendChild(donateBtn);

      // Import/Export
      var importExportBtn = window.UI.iconButton({
        icon: "ImportExport",
        title: "Импорт и экспорт",
        size: 22,
        onClick: showImportExportDialog
      });
      actions.appendChild(importExportBtn);

      // Quick Settings
      var settingsBtn = window.UI.iconButton({
        icon: "Settings",
        title: "Быстрые настройки",
        size: 22,
        onClick: showQuickSettingsDialog
      });
      actions.appendChild(settingsBtn);
    }
  }

  function showSupportDeveloperDialog() {
    var content = window.UI.h("div", { style: { display: "flex", flexDirection: "column", gap: "10px" } });
    var p1 = window.UI.h("p", {
      className: "body-medium",
      text: "Спасибо amurcanov — автору оригинального csqtt."
    });
    var p2 = window.UI.h("p", {
      className: "body-medium text-muted",
      text: "Порт для Windows находится на ранней стадии разработки."
    });
    content.appendChild(p1);
    content.appendChild(p2);

    window.UI.dialog({
      title: "Благодарность",
      content: content,
      actions: [
        { text: "Закрыть", variant: "text", onClick: function (d) { d.close(); } }
      ]
    });
  }

  function showImportExportDialog() {
    // dialogs.js has the full dialog (export with/without secrets); this one is a fallback.
    if (window.Dialogs && window.Dialogs.showImportExportDialog) {
      window.Dialogs.showImportExportDialog();
      return;
    }
    var content = window.UI.h("div", { style: { display: "flex", flexDirection: "column", gap: "12px" } });

    var importBtn = window.UI.button({
      text: "Импорт настроек (текст JSON или csqtt://)",
      variant: "outlined",
      fullWidth: true,
      onClick: function () {
        var importInput = window.UI.h("textarea", {
          className: "m3-field-input",
          style: {
            width: "100%",
            height: "140px",
            border: "1px solid var(--md-sys-color-outline-variant)",
            borderRadius: "12px",
            padding: "8px",
            fontFamily: "var(--font-mono)",
            fontSize: "12px"
          },
          attrs: { placeholder: "Вставьте JSON или ссылку csqtt://..." }
        });
        window.UI.dialog({
          title: "Импорт конфигурации",
          content: importInput,
          actions: [
            { text: "Отмена", variant: "text", onClick: function (d) { d.close(); } },
            {
              text: "Импортировать",
              variant: "filled",
              onClick: function (d) {
                var text = importInput.value.trim();
                if (!text) return;
                window.csqtt.importText(text).then(function () {
                  window.UI.toast("Конфигурация успешно импортирована");
                  d.close();
                  refreshSettings();
                }).catch(function (err) {
                  window.UI.toast("Ошибка импорта: " + (err.message || err));
                });
              }
            }
          ]
        });
      }
    });
    content.appendChild(importBtn);

    var exportBtn = window.UI.button({
      text: "Экспорт настроек в JSON",
      variant: "outlined",
      fullWidth: true,
      onClick: function () {
        window.csqtt.exportJson(true).then(function (json) {
          if (navigator.clipboard && navigator.clipboard.writeText) {
            navigator.clipboard.writeText(json);
            window.UI.toast("JSON конфигурации скопирован в буфер");
          } else {
            window.UI.infoDialog("Экспорт", json);
          }
        }).catch(function (err) {
          window.UI.toast("Ошибка экспорта: " + (err.message || err));
        });
      }
    });
    content.appendChild(exportBtn);

    window.UI.dialog({
      title: "Импорт и экспорт",
      content: content,
      actions: [
        { text: "Закрыть", variant: "text", onClick: function (d) { d.close(); } }
      ]
    });
  }

  function showQuickSettingsDialog() {
    if (!state.settings) return;

    var content = window.UI.h("div", { style: { display: "flex", flexDirection: "column", gap: "10px" } });

    function rebuild() {
      content.innerHTML = "";
      var prof = profile();
      if (!state.settings) return;

      // Role selector (Owner / Participant)
      var roleLabel = window.UI.h("span", { className: "label-medium text-muted", text: "Ваша роль использования" });
      content.appendChild(roleLabel);

      var currentRole = prof && prof.csqtt_link_mode ? "participant" : "owner";
      var roleSeg = window.UI.segmented({
        options: [
          { key: "owner", label: "Владелец" },
          { key: "participant", label: "Участник" }
        ],
        selected: currentRole,
        onSelected: function (role) {
          saveSettings(function (s) {
            if (!s.profiles) s.profiles = [];
            var idx = s.active_profile || 0;
            if (!s.profiles[idx]) s.profiles[idx] = {};
            s.profiles[idx].csqtt_link_mode = (role === "participant");
          }).then(function () {
            rebuild();
          }).catch(function () {});
        }
      });
      content.appendChild(roleSeg);
      content.appendChild(window.UI.divider());

      // Profile selector if not link mode
      if (!prof || !prof.csqtt_link_mode) {
        var profileOptions = (state.settings.profiles || []).map(function (item, idx) {
          return {
            key: String(idx),
            label: item.profile_name && item.profile_name.trim() ? item.profile_name.trim() : ("Профиль " + idx)
          };
        });

        var profDropdown = window.UI.dropdown({
          title: "Профиль конфигурации",
          selectedKey: String(state.settings.active_profile || 0),
          options: profileOptions,
          onSelected: function (key) {
            var nextIdx = parseInt(key, 10);
            saveSettings(function (s) {
              s.active_profile = nextIdx;
            }).then(function () {
              rebuild();
            }).catch(function () {});
          }
        });
        content.appendChild(profDropdown);
        var ownerRow = window.UI.h("div", { style: { display: "flex", gap: "8px" } });
        ownerRow.appendChild(window.UI.button({
          text: "Имя профиля", variant: "outlined",
          onClick: function () {
            var idx = state.settings.active_profile || 0;
            var cur = (state.settings.profiles[idx] || {}).profile_name || ("Профиль " + idx);
            window.SettingsDialogs.profileName(idx, cur, function (i, name) {
              saveSettings(function (s) { if (s.profiles[i]) s.profiles[i].profile_name = name; }).then(rebuild).catch(function () {});
            });
          }
        }));
        ownerRow.appendChild(window.UI.button({
          text: "Выбор TURN подсетей", variant: "outlined",
          onClick: function () {
            var p = profile() || {};
            window.SettingsDialogs.turnSubnets(p.selected_turn_subnet, p.turn_subnet_ip_limit, function (subId, limit) {
              saveSettings(function (s) {
                var pr = s.profiles[s.active_profile || 0];
                if (pr) { pr.selected_turn_subnet = subId; pr.turn_subnet_ip_limit = limit; }
              }).then(function () { window.UI.toast("Настройки подсетей TURN сохранены"); });
            });
          }
        }));
        content.appendChild(ownerRow);
      }

      // Fingerprint
      var fingerprintDropdown = window.UI.dropdown({
        title: "Отпечаток",
        selectedKey: (prof && prof.selected_fingerprint) || "firefox",
        options: [
          { key: "chrome", label: "Chrome" },
          { key: "safari", label: "Safari" },
          { key: "firefox", label: "Firefox" },
          { key: "edge", label: "Edge" },
          { key: "opera", label: "Opera" }
        ],
        onSelected: function (fp) {
          saveSettings(function (s) {
            if (!s.profiles) s.profiles = [];
            var idx = s.active_profile || 0;
            if (!s.profiles[idx]) s.profiles[idx] = {};
            s.profiles[idx].selected_fingerprint = fp;
          });
        }
      });
      content.appendChild(fingerprintDropdown);

      // Theme selector
      var themeDropdown = window.UI.dropdown({
        title: "Оформление",
        selectedKey: state.settings.theme_mode || "system",
        options: [
          { key: "system", label: "Система" },
          { key: "light", label: "Светлая" },
          { key: "dark", label: "Тёмная" }
        ],
        onSelected: function (theme) {
          saveSettings(function (s) {
            s.theme_mode = theme;
          });
        }
      });
      content.appendChild(themeDropdown);

      // Auto-pause Wi-Fi
      var autoPauseRow = window.UI.settingRow({
        title: "Автопауза при Wi-Fi",
        description: "Приостанавливать туннель при подключении к Wi-Fi",
        checked: !!state.settings.auto_pause_on_wifi,
        onCheckedChange: function (checked) {
          saveSettings(function (s) {
            s.auto_pause_on_wifi = checked;
          });
        },
        onInfoClick: function () {
          window.UI.infoDialog("Автопауза при Wi-Fi", "Позволяет временно отключать VPN при нахождении в доверенной домашней или рабочей сети Wi-Fi.");
        }
      });
      content.appendChild(autoPauseRow);

      // Show logs in link mode
      if (prof && prof.csqtt_link_mode) {
        var showLogsRow = window.UI.settingRow({
          title: "Показывать логи",
          checked: !!prof.participant_show_logs,
          onCheckedChange: function (checked) {
            saveSettings(function (s) {
              if (!s.profiles) s.profiles = [];
              var idx = s.active_profile || 0;
              if (!s.profiles[idx]) s.profiles[idx] = {};
              s.profiles[idx].participant_show_logs = checked;
            });
          }
        });
        content.appendChild(showLogsRow);
      }
    }

    rebuild();

    window.UI.dialog({
      title: "Настройки",
      content: content,
      actions: [
        { text: "Готово", variant: "filled", onClick: function (d) { d.close(); } }
      ]
    });
  }

  // Subscribe to csqtt events
  window.csqtt.on("service", function (data) {
    state.serviceRunning = !!data.running;
    updateServiceBanner(state.serviceRunning);
    notifyState();
  });

  window.csqtt.on("status", function (data) {
    if (data && data.status) {
      state.status = data.status;
      notifyState();
    }
  });

  window.csqtt.on("captcha", function (data) {
    state.captchaActive = !!(data && data.active);
    notifyState();
  });

  window.csqtt.on("settings", function (data) {
    if (data && data.settings) {
      state.settings = data.settings;
      applyTheme(state.settings.theme_mode);
      notifyState();
    }
  });

  window.csqtt.on("log", function (data) {
    if (data && data.line) {
      var line = String(data.line);
      var isErr = line.indexOf("ERR") !== -1 || line.indexOf("ОШИБКА") !== -1 || line.indexOf("ФАТАЛ") !== -1;
      if (isErr && currentScreenId !== "logs") {
        state.unreadErrors++;
        // Only update badges in-place; never rebuild buttons just for a badge change
        updateNavBadges();
      }
    }
  });

  document.addEventListener("DOMContentLoaded", function () {
    initHeader();
    refreshSettings();
    window.csqtt.status().then(function (st) {
      state.status = st;
      notifyState();
    }).catch(function () {});
  });

  window.App = {
    state: state,
    onState: onState,
    profile: profile,
    refreshSettings: refreshSettings,
    saveSettings: saveSettings,
    registerScreen: registerScreen,
    showScreen: showScreen,
    showQuickSettingsDialog: showQuickSettingsDialog,
    showSupportDeveloperDialog: showSupportDeveloperDialog,
    showImportExportDialog: showImportExportDialog
  };
})();
