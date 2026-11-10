/**
 * Dialogs Library for csqtt-win
 *
 * Implements window.Dialogs with:
 * - showImportExportDialog / showImportDialog / showExportDialog (mirroring Android ImportExportDialog.kt)
 * - showVkLoginDialog (mirroring Android VkAuthDialog.kt using csqtt.runSelf(["vk-login"]))
 * - showVkLogoutDialog (using csqtt.runSelf(["vk-logout"]))
 * - showVkAccountDialog (unified VK account status and actions)
 *
 * Automatically hooks into App.showImportExportDialog and header actions.
 */

(function () {
  "use strict";

  // ---------------------------------------------------------------------------
  // Import & Export Dialogs (mirroring ImportExportDialog.kt)
  // ---------------------------------------------------------------------------

  function showImportExportDialog(initialTab) {
    var activeTab = (initialTab === "import") ? "import" : "export";
    var includeSecrets = false;
    var currentExportedJson = "";
    var isExportLoading = false;
    var isClosed = false;
    var focusTimer = null;

    var container = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "12px",
        minWidth: "320px",
        maxWidth: "480px"
      }
    });

    var seg = window.UI.segmented({
      options: [
        { key: "export", label: "Экспорт" },
        { key: "import", label: "Импорт" }
      ],
      selected: activeTab,
      onSelected: function (key) {
        activeTab = key;
        updateTabViews();
      }
    });
    container.appendChild(seg);

    // --- Export Pane ---
    var exportPane = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "10px"
      }
    });

    var secretsRow = window.UI.settingRow({
      title: "Включать секреты и пароли",
      description: "Экспорт паролей подключения и токенов",
      checked: includeSecrets,
      onCheckedChange: function (checked) {
        includeSecrets = checked;
        fetchExportData();
      }
    });
    exportPane.appendChild(secretsRow);

    var exportTextarea = window.UI.h("textarea", {
      className: "m3-field-input",
      attrs: {
        readonly: "readonly",
        spellcheck: "false",
        placeholder: "Загрузка конфигурации..."
      },
      style: {
        width: "100%",
        height: "170px",
        fontFamily: "var(--font-mono)",
        fontSize: "11px",
        lineHeight: "16px",
        padding: "10px",
        borderRadius: "12px",
        border: "1px solid var(--md-sys-color-outline-variant)",
        backgroundColor: "var(--terminal-bg)",
        color: "var(--terminal-text)",
        resize: "vertical",
        userSelect: "text"
      }
    });
    exportPane.appendChild(exportTextarea);

    var copyBtn = window.UI.button({
      text: "Копировать в буфер",
      icon: "ContentCopy",
      variant: "filled",
      fullWidth: true,
      onClick: function () {
        if (!currentExportedJson) return;
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(currentExportedJson).then(function () {
            window.UI.toast("Конфиг скопирован в буфер ✓");
          }).catch(function (err) {
            window.UI.toast("Ошибка копирования: " + (err.message || err));
          });
        } else {
          window.UI.toast("Буфер обмена недоступен");
        }
      }
    });
    exportPane.appendChild(copyBtn);
    container.appendChild(exportPane);

    function fetchExportData() {
      if (isExportLoading) return;
      isExportLoading = true;
      exportTextarea.value = "Экспорт данных из службы...";
      copyBtn.disabled = true;

      window.csqtt.exportJson(includeSecrets).then(function (json) {
        if (isClosed) return;
        isExportLoading = false;
        copyBtn.disabled = false;
        currentExportedJson = json || "{}";
        exportTextarea.value = currentExportedJson;
      }).catch(function (err) {
        if (isClosed) return;
        isExportLoading = false;
        copyBtn.disabled = false;
        currentExportedJson = "";
        exportTextarea.value = "Ошибка экспорта: " + (err.message || err);
      });
    }

    // --- Import Pane ---
    var importPane = window.UI.h("div", {
      style: {
        display: "none",
        flexDirection: "column",
        gap: "10px"
      }
    });

    var importNotice = window.UI.h("div", {
      className: "body-small text-muted",
      text: "Вставьте JSON файл конфигурации или ссылку подключения csqtt://"
    });
    importPane.appendChild(importNotice);

    var importTextarea = window.UI.h("textarea", {
      className: "m3-field-input",
      attrs: {
        placeholder: "Вставьте JSON или csqtt://connect?host=...",
        spellcheck: "false"
      },
      style: {
        width: "100%",
        height: "150px",
        fontFamily: "var(--font-mono)",
        fontSize: "12px",
        lineHeight: "16px",
        padding: "10px",
        borderRadius: "12px",
        border: "1px solid var(--md-sys-color-outline-variant)",
        backgroundColor: "var(--md-sys-color-surface-variant)",
        color: "var(--md-sys-color-on-surface)",
        resize: "vertical"
      }
    });
    importPane.appendChild(importTextarea);

    var importActionsRow = window.UI.h("div", {
      style: {
        display: "flex",
        gap: "8px",
        width: "100%"
      }
    });

    var pasteBtn = window.UI.button({
      text: "Из буфера",
      icon: "ContentCopy",
      variant: "outlined",
      className: "compact",
      onClick: function () {
        if (navigator.clipboard && navigator.clipboard.readText) {
          navigator.clipboard.readText().then(function (text) {
            if (isClosed) return;
            if (text && text.trim()) {
              importTextarea.value = text.trim();
              window.UI.toast("Вставлено из буфера ✓");
            } else {
              window.UI.toast("Буфер обмена пуст");
            }
          }).catch(function () {
            if (!isClosed) window.UI.toast("Не удалось прочитать буфер обмена");
          });
        } else {
          importTextarea.focus();
        }
      }
    });
    pasteBtn.style.flex = "1";
    importActionsRow.appendChild(pasteBtn);

    var clearBtn = window.UI.button({
      text: "Очистить",
      variant: "text",
      className: "compact",
      onClick: function () {
        importTextarea.value = "";
      }
    });
    importActionsRow.appendChild(clearBtn);
    importPane.appendChild(importActionsRow);

    var doImportBtn = window.UI.button({
      text: "Импортировать",
      variant: "filled",
      fullWidth: true,
      onClick: function () {
        var raw = (importTextarea.value || "").trim();
        if (!raw) {
          window.UI.toast("Конфигурация пуста");
          return;
        }

        doImportBtn.disabled = true;
        window.csqtt.importText(raw).then(function () {
          if (isClosed) return;
          doImportBtn.disabled = false;
          window.UI.toast("Конфигурация успешно импортирована ✓");
          dlg.close();
          if (window.App && window.App.refreshSettings) {
            window.App.refreshSettings();
          }
        }).catch(function (err) {
          if (isClosed) return;
          doImportBtn.disabled = false;
          window.UI.toast("Ошибка импорта: " + (err.message || err));
        });
      }
    });
    importPane.appendChild(doImportBtn);
    container.appendChild(importPane);

    function updateTabViews() {
      if (activeTab === "export") {
        exportPane.style.display = "flex";
        importPane.style.display = "none";
        fetchExportData();
      } else {
        exportPane.style.display = "none";
        importPane.style.display = "flex";
        if (focusTimer) clearTimeout(focusTimer);
        focusTimer = setTimeout(function () {
          if (!isClosed) {
            importTextarea.focus();
          }
        }, 80);
      }
    }

    var dlg = window.UI.dialog({
      title: "Импорт и Экспорт",
      content: container,
      actions: [
        {
          text: "Закрыть",
          variant: "text",
          onClick: function (d) {
            d.close();
          }
        }
      ],
      onClose: function () {
        isClosed = true;
        if (focusTimer) {
          clearTimeout(focusTimer);
          focusTimer = null;
        }
      }
    });

    updateTabViews();
    return dlg;
  }

  function showImportDialog() {
    return showImportExportDialog("import");
  }

  function showExportDialog() {
    return showImportExportDialog("export");
  }

  // ---------------------------------------------------------------------------
  // VK Login / Logout / Account Dialogs (mirroring VkAuthDialog.kt & vk_auth.rs)
  // ---------------------------------------------------------------------------

  function showVkLoginDialog() {
    var isRunning = true;
    var procHandle = null;
    var isClosed = false;

    var container = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        textAlign: "center",
        gap: "14px",
        padding: "8px 4px",
        minWidth: "280px",
        maxWidth: "420px"
      }
    });

    var headerNote = window.UI.h("p", {
      className: "body-small text-muted",
      style: { lineHeight: "18px" },
      text: "Войдите любым способом, затем токен будет получен автоматически."
    });
    container.appendChild(headerNote);

    var statusCard = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "12px",
        padding: "24px 16px",
        width: "100%",
        borderRadius: "16px",
        backgroundColor: "var(--md-sys-color-surface-variant)"
      }
    });

    var indicatorWrap = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        width: "48px",
        height: "48px"
      }
    });
    var spinner = window.UI.h("div", { className: "m3-spinner" });
    indicatorWrap.appendChild(spinner);
    statusCard.appendChild(indicatorWrap);

    var statusTitle = window.UI.h("span", {
      className: "title-small",
      style: { fontWeight: "600" },
      text: "Запуск авторизации VK…"
    });
    statusCard.appendChild(statusTitle);

    var stepDesc = window.UI.h("span", {
      className: "body-small text-muted",
      style: { minHeight: "18px" },
      text: "Открытие окна браузера…"
    });
    statusCard.appendChild(stepDesc);

    container.appendChild(statusCard);

    var outputLogs = [];

    var dlg = window.UI.dialog({
      title: "Вход в аккаунт ВК",
      content: container,
      actions: [
        {
          text: "Отмена",
          variant: "text",
          onClick: function (d) {
            d.close();
          }
        }
      ],
      onClose: function () {
        isClosed = true;
        isRunning = false;
        if (procHandle && procHandle.kill) {
          try {
            procHandle.kill();
          } catch (e) {}
        }
        procHandle = null;
      }
    });

    try {
      procHandle = window.csqtt.runSelf(["vk-login"], {
        onOutput: function (chunk) {
          if (isClosed || !chunk) return;
          outputLogs.push(chunk);
          var text = String(chunk).trim();
          var lines = text.split(/[\r\n]+/).filter(function (l) { return l.trim().length > 0; });
          var lastLine = lines.length > 0 ? lines[lines.length - 1].trim() : text;

          if (lastLine) {
            stepDesc.textContent = lastLine;
            if (lastLine.indexOf("Получение токена") !== -1) {
              statusTitle.textContent = "Получение токена VK…";
            } else if (lastLine.indexOf("Повторная попытка") !== -1) {
              statusTitle.textContent = "Повторная попытка авторизации…";
            }
          }
        }
      });

      procHandle.done.then(
        function (res) {
          if (isClosed) return;
          isRunning = false;
          var code = res && res.code !== undefined ? res.code : 0;
          var fullLog = outputLogs.join("").trim();

          if (code === 0) {
            indicatorWrap.innerHTML = "";
            indicatorWrap.appendChild(window.Icons.el("CheckCircle", 40, { fill: "var(--extended-success, #356A4C)" }));
            statusTitle.textContent = "Авторизация успешна ✓";
            statusTitle.style.color = "var(--md-sys-color-primary)";

            var idMatch = fullLog.match(/\b\d{5,12}\b/);
            var userIdText = idMatch ? "Пользователь ID: " + idMatch[0] : "Токен успешно получен и сохранён";
            stepDesc.textContent = userIdText;

            window.UI.toast("Вечный VK access token сохранен ✓");
            if (window.App && window.App.refreshSettings) {
              window.App.refreshSettings();
            }

            // Replace actions with "Готово" button
            var actionsContainer = dlg.el.querySelector(".dialog-actions");
            if (actionsContainer) {
              actionsContainer.innerHTML = "";
              var doneBtn = window.UI.button({
                text: "Готово",
                variant: "filled",
                onClick: function () { dlg.close(); }
              });
              actionsContainer.appendChild(doneBtn);
            }
          } else {
            indicatorWrap.innerHTML = "";
            indicatorWrap.appendChild(window.Icons.el("Close", 36, { fill: "var(--md-sys-color-error)" }));
            statusTitle.textContent = "Авторизация отменена";
            statusTitle.style.color = "var(--md-sys-color-error)";
            stepDesc.textContent = fullLog || "Окно авторизации было закрыто.";

            var actContainer = dlg.el.querySelector(".dialog-actions");
            if (actContainer) {
              actContainer.innerHTML = "";
              var retryBtn = window.UI.button({
                text: "Повторить",
                variant: "filled",
                onClick: function () {
                  dlg.close();
                  showVkLoginDialog();
                }
              });
              var closeBtn = window.UI.button({
                text: "Закрыть",
                variant: "text",
                onClick: function () { dlg.close(); }
              });
              actContainer.appendChild(closeBtn);
              actContainer.appendChild(retryBtn);
            }
          }
        },
        function (err) {
          if (isClosed) return;
          isRunning = false;
          indicatorWrap.innerHTML = "";
          indicatorWrap.appendChild(window.Icons.el("Close", 36, { fill: "var(--md-sys-color-error)" }));
          statusTitle.textContent = "Ошибка авторизации";
          statusTitle.style.color = "var(--md-sys-color-error)";
          stepDesc.textContent = (err && err.message) || String(err);
        }
      );
    } catch (e) {
      isRunning = false;
      window.UI.toast("Ошибка запуска процесса VK: " + (e.message || e));
      dlg.close();
    }

    return dlg;
  }

  function showVkLogoutDialog() {
    return window.UI.confirm({
      title: "Выход из VK",
      message: "Вы уверены, что хотите выйти из аккаунта VK? Сохранённый токен будет удалён.",
      confirmText: "Выйти",
      cancelText: "Отмена"
    }).then(function (confirmed) {
      if (!confirmed) return false;

      window.UI.toast("Сброс сессии VK...");
      var proc = window.csqtt.runSelf(["vk-logout"]);
      return proc.done.then(
        function (res) {
          if (res && res.code === 0) {
            window.UI.toast("Сессия VK успешно сброшена ✓");
            if (window.App && window.App.refreshSettings) {
              window.App.refreshSettings();
            }
            return true;
          }
          window.UI.toast("Не удалось сбросить сессию VK");
          return false;
        },
        function (err) {
          window.UI.toast("Ошибка: " + (err.message || err));
          return false;
        }
      );
    });
  }

  function showVkAccountDialog() {
    var prof = (window.App && window.App.profile && window.App.profile()) || {};
    var userId = (prof && prof.vk_access_token_user_id) || "";

    if (!userId) {
      return showVkLoginDialog();
    }

    var isClosed = false;
    var content = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "12px",
        minWidth: "280px"
      }
    });

    var p1 = window.UI.h("p", {
      className: "body-large",
      text: "Авторизован аккаунт VK"
    });
    content.appendChild(p1);

    var userBox = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        gap: "8px",
        padding: "12px",
        borderRadius: "12px",
        backgroundColor: "var(--md-sys-color-surface-variant)"
      }
    });
    userBox.appendChild(window.Icons.el("Login", 20));
    var userText = window.UI.h("span", {
      className: "label-large text-primary",
      text: "ID пользователя: " + userId
    });
    userBox.appendChild(userText);
    content.appendChild(userBox);

    var hint = window.UI.h("p", {
      className: "body-small text-muted",
      text: "Токен используется для автоматического получения звонков ВКонтакте."
    });
    content.appendChild(hint);

    var dlg = window.UI.dialog({
      title: "Аккаунт VK",
      content: content,
      actions: [
        {
          text: "Выйти",
          variant: "outlined",
          onClick: function (d) {
            d.close();
            showVkLogoutDialog();
          }
        },
        {
          text: "Сменить аккаунт",
          variant: "filled",
          onClick: function (d) {
            d.close();
            showVkLoginDialog();
          }
        },
        {
          text: "Закрыть",
          variant: "text",
          onClick: function (d) {
            d.close();
          }
        }
      ],
      onClose: function () {
        isClosed = true;
      }
    });

    return dlg;
  }

  // ---------------------------------------------------------------------------
  // Automatic Hooking & Integration
  // ---------------------------------------------------------------------------

  function setupHeaderInterceptors() {
    if (window.App) {
      window.App.showImportExportDialog = function () {
        showImportExportDialog();
      };
      // Prevent opening amurcanov donation pages or telegram links from header button
      window.App.showSupportDeveloperDialog = function () {
        window.UI.infoDialog(
          "О разработчике",
          "Спасибо amurcanov — автору оригинального csqtt."
        );
      };
    }

    var header = document.getElementById("header-actions");
    if (header) {
      var importExportBtn = header.querySelector('button[title="Импорт и экспорт"]');
      if (importExportBtn) {
        importExportBtn.addEventListener("click", function (e) {
          e.stopImmediatePropagation();
          showImportExportDialog();
        }, true);
      }
      var donateBtn = header.querySelector('button[title="Поддержать разработчика"]');
      if (donateBtn) {
        donateBtn.addEventListener("click", function (e) {
          e.stopImmediatePropagation();
          window.UI.infoDialog(
            "О разработчике",
            "Спасибо amurcanov — автору оригинального csqtt."
          );
        }, true);
      }
    }
  }

  if (window.App) {
    window.App.showImportExportDialog = function () {
      showImportExportDialog();
    };
    window.App.showSupportDeveloperDialog = function () {
      window.UI.infoDialog(
        "О разработчике",
        "Спасибо amurcanov — автору оригинального csqtt."
      );
    };
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", setupHeaderInterceptors);
  } else {
    setupHeaderInterceptors();
  }

  // ---------------------------------------------------------------------------
  // Public API
  // ---------------------------------------------------------------------------

  window.Dialogs = {
    showImportDialog: showImportDialog,
    showExportDialog: showExportDialog,
    showImportExportDialog: showImportExportDialog,
    showVkLoginDialog: showVkLoginDialog,
    showVkLogoutDialog: showVkLogoutDialog,
    showVkAccountDialog: showVkAccountDialog,
    // Convenient aliases
    import: showImportDialog,
    export: showExportDialog,
    importExport: showImportExportDialog,
    vkLogin: showVkLoginDialog,
    vkLogout: showVkLogoutDialog,
    vkAccount: showVkAccountDialog
  };
})();
