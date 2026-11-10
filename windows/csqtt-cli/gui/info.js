/**
 * Info / About Screen for csqtt-win
 *
 * Implements the "Инфо" tab mirroring Android InfoTab.kt,
 * with app version from csqtt.version(),
 * expandable action and project cards, support report copying,
 * and update check via csqtt.runSelf(["update-check"]).
 */

(function () {
  "use strict";

  var cachedVersion = "v...";
  var isCheckingUpdates = false;
  var unsubState = null;
  var currentUpdateSubtitleSpan = null;

  function buildSupportReport() {
    var st = (window.App && window.App.state && window.App.state.status) || {};
    var prof = (window.App && window.App.profile && window.App.profile()) || {};
    var settings = (window.App && window.App.state && window.App.state.settings) || {};
    var lines = [
      "=== CSQTT Support Report ===",
      "App: csqtt-win " + cachedVersion,
      "Platform: Windows (WebView2)",
      "User-Agent: " + (navigator.userAgent || "unknown"),
      "Service Running: " + (window.App && window.App.state ? window.App.state.serviceRunning : "unknown"),
      "Tunnel State: " + (st.state || "stopped"),
      "Tunnel Message: " + (st.message || "none"),
      "Client IP: " + (st.client_ip || "—"),
      "DNS: " + (st.dns || "—"),
      "Active Workers: " + (st.active_workers || 0),
      "Active Profile: " + (settings.active_profile !== undefined ? settings.active_profile : 0),
      "Link Mode: " + (prof.csqtt_link_mode ? "true" : "false"),
      "Auth Mode: " + (prof.vk_auth_mode || "vkcalls"),
      "Hash Mode: " + (prof.vk_hash_mode || "manual"),
      "SRTP Mode: " + (prof.srtp_mode || "auto"),
      "Transport: " + (prof.turn_transport || "udp"),
      "Generated At: " + new Date().toISOString()
    ];
    return lines.join("\n");
  }

  function runUpdateCheck(statusSubtitleEl, actionBtnEl) {
    if (isCheckingUpdates) return;
    isCheckingUpdates = true;
    if (statusSubtitleEl) {
      statusSubtitleEl.textContent = "Проверяем GitHub releases...";
      statusSubtitleEl.style.color = "var(--md-sys-color-primary)";
    }
    if (actionBtnEl) actionBtnEl.disabled = true;

    window.UI.toast("Проверка обновлений...");

    var outputChunks = [];
    var proc = window.csqtt.runSelf(["update-check"], {
      onOutput: function (chunk) {
        outputChunks.push(chunk);
      }
    });

    proc.done.then(
      function () {
        isCheckingUpdates = false;
        if (actionBtnEl) actionBtnEl.disabled = false;
        var fullText = outputChunks.join("").trim();

        if (statusSubtitleEl) {
          if (fullText.indexOf("Доступно обновление") !== -1) {
            statusSubtitleEl.textContent = "Доступно обновление на GitHub";
            statusSubtitleEl.style.color = "var(--md-sys-color-primary)";
          } else if (fullText.indexOf("актуальная версия") !== -1) {
            statusSubtitleEl.textContent = "У вас последняя версия";
            statusSubtitleEl.style.color = "var(--md-sys-color-on-surface-variant)";
          } else {
            statusSubtitleEl.textContent = "Проверка завершена";
            statusSubtitleEl.style.color = "var(--md-sys-color-on-surface-variant)";
          }
        }

        var pre = window.UI.h("pre", {
          style: {
            fontFamily: "var(--font-mono)",
            fontSize: "12px",
            lineHeight: "18px",
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            maxHeight: "340px",
            overflow: "auto",
            padding: "12px",
            borderRadius: "12px",
            backgroundColor: "var(--terminal-bg)",
            color: "var(--terminal-text)"
          },
          text: fullText || "Проверка завершена без вывода сообщений."
        });

        window.UI.infoDialog("Проверка обновлений", pre);
      },
      function (err) {
        isCheckingUpdates = false;
        if (actionBtnEl) actionBtnEl.disabled = false;
        if (statusSubtitleEl) {
          statusSubtitleEl.textContent = "Ошибка при проверке обновлений";
          statusSubtitleEl.style.color = "var(--md-sys-color-error)";
        }
        window.UI.toast("Ошибка проверки обновлений: " + (err.message || err));
      }
    );
  }

  function createActionTile(opts) {
    opts = opts || {};
    var isEnabled = opts.enabled !== false;
    var row = window.UI.h("div", {
      className: "setting-row" + (isEnabled ? "" : " disabled"),
      style: {
        cursor: isEnabled ? "pointer" : "default",
        opacity: isEnabled ? "1" : "0.6",
        padding: "10px 12px",
        borderRadius: "16px",
        backgroundColor: "var(--md-sys-color-surface-variant)",
        transition: "background-color var(--motion-fast) var(--ease-standard)"
      },
      attrs: { role: isEnabled ? "button" : "presentation" },
      on: {
        click: function (e) {
          if (isEnabled && opts.onClick) opts.onClick(e);
        }
      }
    });

    var leftCol = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        gap: "12px",
        flex: "1",
        minWidth: "0"
      }
    });

    if (opts.icon) {
      var iconWrap = window.UI.h("div", {
        style: {
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: "36px",
          height: "36px",
          borderRadius: "50%",
          backgroundColor: "var(--card-background)",
          color: "var(--md-sys-color-primary)",
          flexShrink: "0"
        }
      });
      var iconEl = typeof opts.icon === "string" ? window.Icons.el(opts.icon, 20) : opts.icon;
      iconWrap.appendChild(iconEl);
      leftCol.appendChild(iconWrap);
    }

    var textWrap = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "2px",
        flex: "1",
        minWidth: "0"
      }
    });

    var titleEl = window.UI.h("span", {
      className: "title-small",
      style: { fontWeight: "600", color: "var(--md-sys-color-on-surface)" },
      text: opts.title || ""
    });
    textWrap.appendChild(titleEl);

    var subtitleEl = null;
    if (opts.subtitle) {
      subtitleEl = window.UI.h("span", {
        className: "body-small text-muted",
        style: { wordBreak: "break-word" },
        text: opts.subtitle
      });
      textWrap.appendChild(subtitleEl);
    }
    leftCol.appendChild(textWrap);
    row.appendChild(leftCol);

    if (isEnabled) {
      var trailing = window.UI.h("div", {
        style: {
          color: "var(--md-sys-color-on-surface-variant)",
          display: "flex",
          alignItems: "center",
          flexShrink: "0",
          paddingLeft: "8px",
          transform: "rotate(-90deg)"
        }
      });
      trailing.appendChild(window.Icons.el("ChevronDown", 20));
      row.appendChild(trailing);
    }

    row.setSubtitle = function (txt) {
      if (subtitleEl) subtitleEl.textContent = txt || "";
    };
    return row;
  }

  function renderInfoScreen(container) {
    if (unsubState) {
      unsubState();
      unsubState = null;
    }
    container.innerHTML = "";

    // 1. Hero Section Card
    var heroCard = window.UI.sectionCard();
    heroCard.style.alignItems = "center";
    heroCard.style.textAlign = "center";
    heroCard.style.padding = "24px 20px";

    var logoWrap = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        marginBottom: "4px"
      }
    });
    logoWrap.appendChild(window.Icons.el("CsqttLogo", 46));
    heroCard.appendChild(logoWrap);

    var title = window.UI.h("div", {
      className: "title-large",
      style: { fontWeight: "bold", letterSpacing: "0.5px" },
      text: "CSQTT"
    });
    heroCard.appendChild(title);

    var subtitle = window.UI.h("div", {
      className: "body-small text-muted",
      text: "Клиент VPN для Windows на базе WebRTC"
    });
    heroCard.appendChild(subtitle);

    var versionChip = window.UI.h("div", {
      className: "m3-chip selected",
      style: {
        marginTop: "8px",
        marginBottom: "16px",
        pointerEvents: "none"
      }
    });
    var versionText = window.UI.h("span", { text: cachedVersion });
    versionChip.appendChild(versionText);
    heroCard.appendChild(versionChip);

    // Initial fetch of version
    window.csqtt.version().then(function (ver) {
      if (ver) {
        cachedVersion = "v" + String(ver).replace(/^v/, "");
        versionText.textContent = cachedVersion;
      }
    }).catch(function () {});

    // Thanks line replacing "Поддержать разработчика"
    var thanksLine = window.UI.h("div", {
      className: "body-small text-muted",
      style: {
        textAlign: "center",
        lineHeight: "20px",
        marginBottom: "12px",
        maxWidth: "360px"
      },
      text: "Спасибо amurcanov — автору оригинального csqtt"
    });
    heroCard.appendChild(thanksLine);

    var heroButtons = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "10px",
        width: "100%",
        maxWidth: "360px"
      }
    });

    var tgBtn = window.UI.button({
      text: "Telegram канал — скоро",
      icon: "Link",
      variant: "outlined",
      fullWidth: true,
      enabled: false
    });
    heroButtons.appendChild(tgBtn);

    var supportBtn = window.UI.button({
      text: "Поддержка — скоро",
      icon: "HelpOutline",
      variant: "outlined",
      fullWidth: true,
      enabled: false
    });
    heroButtons.appendChild(supportBtn);

    heroCard.appendChild(heroButtons);
    container.appendChild(heroCard);

    // Initial expanded state from settings
    var appSettings = (window.App && window.App.state && window.App.state.settings) || {};
    var actionsExpanded = appSettings.info_actions_expanded !== false;
    var projectExpanded = appSettings.info_project_expanded !== false;

    // 2. Actions Card ("Действия")
    var actionsCard = window.UI.sectionCard();

    var actionsHeader = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        cursor: "pointer",
        userSelect: "none"
      }
    });

    var actionsTitleRow = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", gap: "8px" }
    });
    actionsTitleRow.appendChild(window.Icons.el("Info", 18, { fill: "var(--md-sys-color-primary)" }));
    actionsTitleRow.appendChild(window.UI.h("span", {
      className: "title-medium",
      text: "Действия"
    }));
    actionsHeader.appendChild(actionsTitleRow);

    var actionsChevron = window.Icons.el("ChevronDown", 20);
    actionsChevron.style.transition = "transform var(--motion-fast) var(--ease-standard)";
    actionsChevron.style.transform = actionsExpanded ? "rotate(180deg)" : "rotate(0deg)";
    actionsHeader.appendChild(actionsChevron);
    actionsCard.appendChild(actionsHeader);

    var actionsList = window.UI.h("div", {
      style: {
        display: actionsExpanded ? "flex" : "none",
        flexDirection: "column",
        gap: "10px"
      }
    });

    var supportTile = createActionTile({
      title: "Поддержка — скоро",
      subtitle: "Обратная связь появится позже",
      icon: "HelpOutline",
      enabled: false
    });
    actionsList.appendChild(supportTile);

    var reportTile = createActionTile({
      title: "Собрать отчёт",
      subtitle: "ОС, архитектура, версия, состояние",
      icon: "ContentCopy",
      onClick: function () {
        var rep = buildSupportReport();
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(rep).then(function () {
            window.UI.toast("Отчёт сформирован и скопирован ✓");
          }).catch(function () {
            window.UI.infoDialog("Отчёт CSQTT", rep);
          });
        } else {
          window.UI.infoDialog("Отчёт CSQTT", rep);
        }
      }
    });
    actionsList.appendChild(reportTile);

    var updateTile = createActionTile({
      title: "Проверить обновления",
      subtitle: "Проверить GitHub вручную",
      icon: "ArrowDownward",
      onClick: function () {
        runUpdateCheck(updateSubtitleSpan, updateTile);
      }
    });
    var updateSubtitleSpan = updateTile.querySelector(".body-small");
    currentUpdateSubtitleSpan = updateSubtitleSpan;
    actionsList.appendChild(updateTile);

    // Portable install: stops the service (tunnel, routes and DNS are removed) and deletes it.
    var uninstallTile = createActionTile({
      title: "Удалить службу",
      subtitle: "Остановить и удалить службу csqtt-win (нужны права администратора)",
      icon: "Close",
      onClick: function () {
        window.UI.confirm({
          title: "Удалить службу?",
          message: "VPN отключится, служба csqtt-win будет остановлена и удалена. Файлы программы и настройки останутся.",
          confirmText: "Удалить",
          cancelText: "Отмена"
        }).then(function (ok) {
          if (!ok) return;
          window.UI.toast("Удаление службы…");
          window.csqtt.uninstallService().then(
            function () { window.UI.toast("Служба удалена. Теперь можно удалить папку программы."); },
            function (err) { window.UI.toast((err && err.message) || String(err)); }
          );
        });
      }
    });
    actionsList.appendChild(uninstallTile);

    actionsCard.appendChild(actionsList);
    container.appendChild(actionsCard);

    actionsHeader.addEventListener("click", function () {
      actionsExpanded = !actionsExpanded;
      actionsList.style.display = actionsExpanded ? "flex" : "none";
      actionsChevron.style.transform = actionsExpanded ? "rotate(180deg)" : "rotate(0deg)";
      if (window.App && window.App.saveSettings) {
        window.App.saveSettings(function (s) {
          s.info_actions_expanded = actionsExpanded;
        }).catch(function () {});
      }
    });

    // 3. Project Card ("О проекте")
    var projectCard = window.UI.sectionCard();

    var projectHeader = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        cursor: "pointer",
        userSelect: "none"
      }
    });

    var projectTitleRow = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", gap: "8px" }
    });
    projectTitleRow.appendChild(window.Icons.el("Tag", 18, { fill: "var(--md-sys-color-primary)" }));
    projectTitleRow.appendChild(window.UI.h("span", {
      className: "title-medium",
      text: "О проекте"
    }));
    projectHeader.appendChild(projectTitleRow);

    var projectChevron = window.Icons.el("ChevronDown", 20);
    projectChevron.style.transition = "transform var(--motion-fast) var(--ease-standard)";
    projectChevron.style.transform = projectExpanded ? "rotate(180deg)" : "rotate(0deg)";
    projectHeader.appendChild(projectChevron);
    projectCard.appendChild(projectHeader);

    var projectList = window.UI.h("div", {
      style: {
        display: projectExpanded ? "flex" : "none",
        flexDirection: "column",
        gap: "10px"
      }
    });

    var descTile = window.UI.h("div", {
      style: {
        padding: "12px 14px",
        borderRadius: "16px",
        backgroundColor: "var(--md-sys-color-surface-variant)",
        display: "flex",
        flexDirection: "column",
        gap: "4px"
      }
    });
    descTile.appendChild(window.UI.h("span", {
      className: "title-small",
      style: { fontWeight: "600", color: "var(--md-sys-color-on-surface)" },
      text: "csqtt-win"
    }));
    descTile.appendChild(window.UI.h("span", {
      className: "body-small text-muted",
      text: "Windows-порт csqtt (ранняя версия)"
    }));
    projectList.appendChild(descTile);

    var thanksTile = window.UI.h("div", {
      style: {
        padding: "12px 14px",
        borderRadius: "16px",
        backgroundColor: "var(--md-sys-color-surface-variant)",
        display: "flex",
        flexDirection: "column",
        gap: "4px"
      }
    });
    thanksTile.appendChild(window.UI.h("span", {
      className: "title-small",
      style: { fontWeight: "600", color: "var(--md-sys-color-on-surface)" },
      text: "Благодарность"
    }));
    thanksTile.appendChild(window.UI.h("span", {
      className: "body-small text-muted",
      text: "Спасибо amurcanov — автору оригинального csqtt"
    }));
    projectList.appendChild(thanksTile);

    var licenseTile = window.UI.h("div", {
      style: {
        padding: "12px 14px",
        borderRadius: "16px",
        backgroundColor: "var(--md-sys-color-surface-variant)",
        display: "flex",
        flexDirection: "column",
        gap: "4px"
      }
    });
    licenseTile.appendChild(window.UI.h("span", {
      className: "title-small",
      style: { fontWeight: "600", color: "var(--md-sys-color-on-surface)" },
      text: "Лицензия"
    }));
    licenseTile.appendChild(window.UI.h("span", {
      className: "body-small text-muted",
      text: "PolyForm Noncommercial 1.0.0"
    }));
    projectList.appendChild(licenseTile);

    projectCard.appendChild(projectList);
    container.appendChild(projectCard);

    projectHeader.addEventListener("click", function () {
      projectExpanded = !projectExpanded;
      projectList.style.display = projectExpanded ? "flex" : "none";
      projectChevron.style.transform = projectExpanded ? "rotate(180deg)" : "rotate(0deg)";
      if (window.App && window.App.saveSettings) {
        window.App.saveSettings(function (s) {
          s.info_project_expanded = projectExpanded;
        }).catch(function () {});
      }
    });

    // 4. Footer
    var footer = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "6px",
        padding: "12px 0 24px 0",
        width: "100%"
      }
    });

    var licenseText = window.UI.h("span", {
      className: "body-small text-muted",
      style: { opacity: "0.75" },
      text: "csqtt-win • PolyForm Noncommercial 1.0.0"
    });
    footer.appendChild(licenseText);

    container.appendChild(footer);

    // Initial sync
    syncState(window.App && window.App.state);
  }

  function syncState(st) {
    if (!st || !st.settings) return;
    var s = st.settings;
    if (s.update_latest_version && currentUpdateSubtitleSpan && !isCheckingUpdates) {
      currentUpdateSubtitleSpan.textContent = "Последняя версия: " + s.update_latest_version;
    }
  }

  function onShow() {
    if (window.App && window.App.onState) {
      if (unsubState) unsubState();
      unsubState = window.App.onState(syncState);
    }
  }

  function onHide() {
    if (unsubState) {
      unsubState();
      unsubState = null;
    }
  }

  // Register screen with order 5
  if (window.App && window.App.registerScreen) {
    window.App.registerScreen({
      id: "info",
      title: "Инфо",
      icon: "Info",
      order: 5,
      render: renderInfoScreen,
      onShow: onShow,
      onHide: onHide
    });
  }

  // Public API
  window.Info = {
    show: function () {
      if (window.App && window.App.showScreen) {
        window.App.showScreen("info");
      }
    },
    checkUpdates: function () {
      runUpdateCheck();
    },
    buildSupportReport: buildSupportReport
  };
})();
