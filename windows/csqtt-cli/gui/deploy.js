/**
 * Deploy Screen for csqtt-win (Material 3)
 *
 * Implements the server deployment and undeployment screen mirroring Android DeployTab
 * and DeployScreen.kt.
 *
 * Runs `csqtt deploy` and `csqtt undeploy` via `window.csqtt.runSelf(args, { env, onOutput })`.
 * Displays real-time output in a scrollable monospace console keeping the last 2000 lines.
 * Handles interactive TOFU SSH host key verification prompts via UI.confirm.
 *
 * Note: SSH password is kept only in session memory and is NEVER stored in persistent settings.
 */

(function () {
  "use strict";

  var DEFAULT_SSH_PORT = 22;
  var DEFAULT_PEER_PORT = 46000;
  var DEFAULT_WEB_PORT = 46002;
  var MAX_CONSOLE_LINES = 2000;

  var SECRET_DEPLOY_MAIN_PASSWORD = "deploy_main_password_encrypted";
  var SECRET_DEPLOY_WEB_PASSWORD = "deploy_web_password_encrypted";

  // Session-only credentials (never saved to settings file)
  var sessionSshPassword = "";
  var sessionMainPassword = "";
  var sessionWebPassword = "";
  var sessionWebLogin = "";
  var sessionSshKeyPath = "";
  var sessionSshKeyPassphrase = "";

  // Process control state
  var isDeploying = false;
  var activeHandle = null;
  var activeOperationType = ""; // "deploy" | "undeploy"

  // Console state
  var consoleLines = [];
  var consoleViewportEl = null;
  var consoleCountBadgeEl = null;
  var rawOutputBuffer = "";

  // UI elements kept for dynamic state refresh
  var hostField = null;
  var sshLoginField = null;
  var sshPasswordField = null;
  var sshPortField = null;
  var peerPortField = null;
  var webPortField = null;
  var sshKeysModeRow = null;
  var manualPortsRow = null;
  var manualPortsContainer = null;
  var dockerInstallRow = null;
  var sshKeysBtn = null;
  var sshKeysRowWrapper = null;
  var sshPasswordRowWrapper = null;
  var panelUrlInput = null;
  var authStatusBadge = null;
  var installBtn = null;
  var uninstallBtn = null;
  var cancelBtn = null;
  var runningBanner = null;

  // ---------------------------------------------------------------------------
  // Validation and Helpers
  // ---------------------------------------------------------------------------

  function isValidPort(val) {
    var num = parseInt(val, 10);
    return !isNaN(num) && num >= 1 && num <= 65535 && String(num) === String(val).trim();
  }

  function is16AlphaNum(str) {
    return typeof str === "string" && str.length === 16 && /^[a-zA-Z0-9]{16}$/.test(str);
  }

  function isAlphaNum(str) {
    return typeof str === "string" && str.length > 0 && /^[a-zA-Z0-9]+$/.test(str);
  }

  function generate16CharPassword() {
    var alphabet = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    var result = "";
    var buf = new Uint8Array(32);
    while (result.length < 16) {
      if (window.crypto && window.crypto.getRandomValues) {
        window.crypto.getRandomValues(buf);
      } else {
        for (var i = 0; i < buf.length; i++) {
          buf[i] = Math.floor(Math.random() * 256);
        }
      }
      for (var j = 0; j < buf.length; j++) {
        // Discard byte values >= 248 to eliminate modulo bias (62 * 4 = 248)
        if (buf[j] < 248) {
          result += alphabet.charAt(buf[j] % 62);
          if (result.length === 16) break;
        }
      }
    }
    return result;
  }

  function generateUuid() {
    if (window.crypto && window.crypto.randomUUID) {
      return window.crypto.randomUUID();
    }
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, function (c) {
      var r = (Math.random() * 16) | 0;
      var v = c === "x" ? r : (r & 0x3) | 0x8;
      return v.toString(16);
    });
  }

  function computePanelUrl(host, webPort, secretUuid) {
    var h = (host || "").trim() || "IP";
    var p = webPort || DEFAULT_WEB_PORT;
    var u = "";
    if (secretUuid && secretUuid.trim()) {
      var trimmed = secretUuid.trim();
      u = trimmed.charAt(0) === "/" ? trimmed : "/" + trimmed;
    } else {
      u = "/uuid";
    }
    return "https://" + h + ":" + p + u;
  }

  // ---------------------------------------------------------------------------
  // Monospace Console Buffer and Rendering
  // ---------------------------------------------------------------------------

  function appendConsoleOutput(chunk) {
    if (!chunk) return;
    rawOutputBuffer += chunk;

    // Check for interactive SSH host key TOFU prompt
    checkInteractiveTofuPrompt(rawOutputBuffer);

    // Split accumulated buffer into lines while preserving any trailing partial line
    var parts = rawOutputBuffer.split(/\r?\n/);
    // Keep the last part in buffer because it might be an unfinished prompt
    rawOutputBuffer = parts.pop() || "";

    if (parts.length > 0) {
      for (var i = 0; i < parts.length; i++) {
        consoleLines.push(parts[i]);
      }
      if (consoleLines.length > MAX_CONSOLE_LINES) {
        consoleLines = consoleLines.slice(-MAX_CONSOLE_LINES);
      }
      renderConsoleViewport();
    }
  }

  function renderConsoleViewport() {
    if (!consoleViewportEl) return;

    // Fast textContent update using safe string join
    // Includes any pending partial line in buffer so prompts like [y/N]: are visible
    var fullText = consoleLines.join("\n");
    if (rawOutputBuffer) {
      fullText += (fullText.length > 0 ? "\n" : "") + rawOutputBuffer;
    }
    consoleViewportEl.textContent = fullText;

    if (consoleCountBadgeEl) {
      consoleCountBadgeEl.textContent = consoleLines.length + " строк";
    }

    // Auto-scroll to bottom
    consoleViewportEl.scrollTop = consoleViewportEl.scrollHeight;
  }

  function clearConsole() {
    consoleLines = [];
    rawOutputBuffer = "";
    renderConsoleViewport();
  }

  function copyConsole() {
    var text = consoleLines.join("\n");
    if (rawOutputBuffer) {
      text += (text.length > 0 ? "\n" : "") + rawOutputBuffer;
    }
    if (!text) {
      window.UI.toast("Консоль пуста");
      return;
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(function () {
        window.UI.toast("Вывод скопирован в буфер");
      }).catch(function () {
        window.UI.toast("Не удалось скопировать");
      });
    } else {
      window.UI.toast("Буфер обмена недоступен");
    }
  }

  var tofuPromptInFlight = false;
  function checkInteractiveTofuPrompt(bufferText) {
    if (tofuPromptInFlight || !activeHandle) return;

    var isTofu = bufferText.indexOf("Доверять этому ключу?") !== -1 ||
                 bufferText.indexOf("[y/N]") !== -1 ||
                 bufferText.indexOf("Неизвестный ключ SSH") !== -1;

    if (isTofu) {
      tofuPromptInFlight = true;
      window.UI.confirm({
        title: "Неизвестный ключ SSH узла",
        message: "Сервер вернул неизвестный SSH-ключ узла. Доверять этому ключу и сохранить его в known_hosts?",
        confirmText: "Доверять (Да)",
        cancelText: "Отклонить (Нет)"
      }).then(function (accepted) {
        tofuPromptInFlight = false;
        if (activeHandle) {
          activeHandle.input(accepted ? "yes\n" : "no\n");
        }
      }).catch(function () {
        tofuPromptInFlight = false;
        if (activeHandle) {
          activeHandle.input("no\n");
        }
      });
    }
  }

  // ---------------------------------------------------------------------------
  // Dialogs
  // ---------------------------------------------------------------------------

  function showAuthorizationDialog() {
    var p = window.App.profile() || {};
    var currentWebLogin = sessionWebLogin || p.deploy_web_login || "admin";
    var currentMainPass = sessionMainPassword || generate16CharPassword();
    var currentWebPass = sessionWebPassword || "";

    var content = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "14px",
        paddingTop: "6px"
      }
    });

    var desc = window.UI.h("div", {
      style: {
        fontSize: "13px",
        color: "var(--md-sys-color-on-surface-variant)",
        lineHeight: "18px"
      },
      text: "Придумайте логин и пароль для web-панели и мастер-пароль туннеля — латиница и цифры."
    });
    content.appendChild(desc);

    // Section 1: Tunnel Master Password
    var tunnelPassHeader = window.UI.h("div", {
      style: {
        fontSize: "14px",
        fontWeight: "600",
        color: "var(--md-sys-color-primary)",
        marginTop: "4px"
      },
      text: "Пароль подключения к туннелю"
    });
    content.appendChild(tunnelPassHeader);

    var mainPassField = window.UI.textField({
      label: "Пароль туннеля (16 символов)",
      placeholder: "16 символов [0-9A-Za-z]",
      value: currentMainPass,
      trailing: window.UI.iconButton({
        icon: "ImportExport",
        title: "Сгенерировать 16-значный пароль",
        size: 18,
        onClick: function () {
          var gen = generate16CharPassword();
          mainPassField.setValue(gen);
          mainPassField.setError(null);
        }
      }),
      onInput: function (val) {
        var clean = val.replace(/\s+/g, "").slice(0, 16);
        if (clean !== val) mainPassField.setValue(clean);
        if (clean.length > 0 && !is16AlphaNum(clean)) {
          mainPassField.setError("Требуется ровно 16 символов [0-9A-Za-z]");
        } else {
          mainPassField.setError(null);
        }
      }
    });
    content.appendChild(mainPassField.el);

    // Section 2: Web Management Panel Credentials
    var webHeader = window.UI.h("div", {
      style: {
        fontSize: "14px",
        fontWeight: "600",
        color: "var(--md-sys-color-primary)",
        marginTop: "6px"
      },
      text: "Данные доступа к web-панели"
    });
    content.appendChild(webHeader);

    var webLoginField = window.UI.textField({
      label: "Логин WEB (придумайте)",
      placeholder: "admin",
      value: currentWebLogin,
      onInput: function (val) {
        var clean = val.replace(/\s+/g, "");
        if (clean !== val) webLoginField.setValue(clean);
        if (clean.length > 0 && !isAlphaNum(clean)) {
          webLoginField.setError("Разрешена латиница и цифры");
        } else {
          webLoginField.setError(null);
        }
      }
    });
    content.appendChild(webLoginField.el);

    var webPassField = window.UI.textField({
      label: "Пароль WEB (придумайте)",
      placeholder: "придумайте пароль",
      value: currentWebPass,
      isPassword: true,
      onInput: function (val) {
        var clean = val.replace(/\s+/g, "");
        if (clean !== val) webPassField.setValue(clean);
        if (clean.length > 0 && !isAlphaNum(clean)) {
          webPassField.setError("Разрешена латиница и цифры");
        } else {
          webPassField.setError(null);
        }
      }
    });
    content.appendChild(webPassField.el);

    var dlg = window.UI.dialog({
      title: "Авторизация",
      content: content,
      actions: [
        {
          text: "Отмена",
          variant: "text",
          onClick: function (d) { d.close(); }
        },
        {
          text: "Сохранить",
          variant: "filled",
          onClick: function (d) {
            var mPass = mainPassField.getValue().trim();
            var wLogin = webLoginField.getValue().trim();
            var wPass = webPassField.getValue().trim();

            if (!is16AlphaNum(mPass)) {
              mainPassField.setError("Требуется ровно 16 символов [0-9A-Za-z]");
              return;
            }
            if (!isAlphaNum(wLogin)) {
              webLoginField.setError("Разрешена латиница и цифры");
              return;
            }
            if (!isAlphaNum(wPass)) {
              webPassField.setError("Разрешена латиница и цифры");
              return;
            }

            sessionMainPassword = mPass;
            sessionWebLogin = wLogin;
            sessionWebPassword = wPass;

            // Persist web login into active profile
            window.App.saveSettings(function (s) {
              var prof = s.profiles[s.active_profile || 0];
              if (prof) {
                prof.deploy_web_login = wLogin;
              }
            });

            // Persist secrets in background via service IPC
            window.csqtt.setSecret(SECRET_DEPLOY_MAIN_PASSWORD, mPass);
            window.csqtt.setSecret(SECRET_DEPLOY_WEB_PASSWORD, wPass);

            updateAuthStatusBadge();
            window.UI.toast("Данные авторизации сохранены");
            d.close();
          }
        }
      ]
    });
  }

  function showSshKeysDialog() {
    var content = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "14px",
        paddingTop: "6px"
      }
    });

    var desc = window.UI.h("div", {
      style: {
        fontSize: "13px",
        color: "var(--md-sys-color-on-surface-variant)",
        lineHeight: "18px"
      },
      text: "Укажите абсолютный путь к приватному ключу SSH (OpenSSH или PEM) и при необходимости пароль расшифровки."
    });
    content.appendChild(desc);

    var keyPathField = window.UI.textField({
      label: "Путь к приватному ключу SSH",
      placeholder: "C:\\Users\\...\\.ssh\\id_ed25519",
      value: sessionSshKeyPath
    });
    content.appendChild(keyPathField.el);

    var passphraseField = window.UI.textField({
      label: "Пароль к ключу (если зашифрован)",
      placeholder: "оставьте пустым, если ключ без пароля",
      value: sessionSshKeyPassphrase,
      isPassword: true
    });
    content.appendChild(passphraseField.el);

    window.UI.dialog({
      title: "Ключи SSH",
      content: content,
      actions: [
        {
          text: "Отмена",
          variant: "text",
          onClick: function (d) { d.close(); }
        },
        {
          text: "Сохранить",
          variant: "filled",
          onClick: function (d) {
            sessionSshKeyPath = keyPathField.getValue().trim();
            sessionSshKeyPassphrase = passphraseField.getValue();
            updateSshKeyButtonState();
            window.UI.toast("Параметры SSH-ключей сохранены");
            d.close();
          }
        }
      ]
    });
  }

  function showUninstallConfirmDialog(onConfirmed) {
    var content = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "12px",
        paddingTop: "4px"
      }
    });

    var warningText = window.UI.h("div", {
      style: {
        fontSize: "13px",
        color: "var(--md-sys-color-on-surface-variant)",
        lineHeight: "18px"
      },
      text: "Будут удалены: бинарник, systemd-сервис, бот, конфигурация CSQTT и только помеченные правила firewall/NAT для CSQTT.\n\nЭто действие необратимо."
    });
    content.appendChild(warningText);

    var confirmField = window.UI.textField({
      label: "Введите «да» для подтверждения",
      placeholder: "да"
    });
    content.appendChild(confirmField.el);

    var dlg = window.UI.dialog({
      title: "Удаление CSQTT с сервера",
      content: content,
      actions: [
        {
          text: "Отмена",
          variant: "text",
          onClick: function (d) { d.close(); }
        },
        {
          text: "Удалить",
          variant: "filled",
          onClick: function (d) {
            var val = confirmField.getValue().trim().toLowerCase();
            if (val === "да" || val === "yes") {
              d.close();
              if (onConfirmed) onConfirmed();
            } else {
              confirmField.setError("Введите слово «да»");
            }
          }
        }
      ]
    });
  }

  // ---------------------------------------------------------------------------
  // Process Execution: csqtt deploy & csqtt undeploy
  // ---------------------------------------------------------------------------

  function startProcess(operation) {
    if (isDeploying) return;

    var p = window.App.profile() || {};
    var host = (hostField ? hostField.getValue() : p.deploy_ip || "").trim();
    var sshLogin = (sshLoginField ? sshLoginField.getValue() : p.deploy_login || "").trim() || "root";
    var sshPassword = sessionSshPassword || "";
    var isSshKeysMode = !!p.ssh_keys_mode;
    var isManualPorts = !!p.manual_ports_enabled;
    var sshPort = isManualPorts ? ((sshPortField ? sshPortField.getValue() : p.deploy_ssh_port) || DEFAULT_SSH_PORT) : DEFAULT_SSH_PORT;
    var peerPort = isManualPorts ? ((peerPortField ? peerPortField.getValue() : p.server_peer_port) || DEFAULT_PEER_PORT) : DEFAULT_PEER_PORT;
    var webPort = isManualPorts ? ((webPortField ? webPortField.getValue() : p.server_web_port) || DEFAULT_WEB_PORT) : DEFAULT_WEB_PORT;
    var dockerInstall = !!p.docker_install;
    var secretUuid = p.deploy_secret_path_uuid || "";
    var webLogin = sessionWebLogin || p.deploy_web_login || "admin";
    var webPassword = sessionWebPassword || "";
    var mainPassword = sessionMainPassword || "";

    // Validation
    if (!host) {
      if (hostField) hostField.setError("Укажите IP сервера или домен");
      window.UI.toast("Укажите IP сервера или домен");
      return;
    }
    if (hostField) hostField.setError(null);

    if (isManualPorts) {
      if (!isValidPort(sshPort) || !isValidPort(peerPort) || !isValidPort(webPort)) {
        window.UI.toast("Проверьте правильность портов (1-65535)");
        return;
      }
    }

    if (!isSshKeysMode && !sshPassword) {
      if (sshPasswordField) sshPasswordField.setError("Укажите SSH-пароль");
      window.UI.toast("Укажите данные SSH для подключения к серверу");
      return;
    }
    if (sshPasswordField) sshPasswordField.setError(null);

    if (isSshKeysMode && !sessionSshKeyPath) {
      window.UI.toast("Укажите путь к приватному SSH-ключу в разделе «Ключи SSH»");
      showSshKeysDialog();
      return;
    }

    if (operation === "deploy") {
      if (!is16AlphaNum(mainPassword)) {
        window.UI.toast("Пароль туннеля должен содержать ровно 16 символов [0-9A-Za-z]");
        showAuthorizationDialog();
        return;
      }
      if (!isAlphaNum(webLogin) || !isAlphaNum(webPassword)) {
        window.UI.toast("Укажите логин и пароль web-панели в разделе «Авторизация»");
        showAuthorizationDialog();
        return;
      }
    }

    // Save latest non-sensitive form inputs into active profile
    window.App.saveSettings(function (s) {
      var prof = s.profiles[s.active_profile || 0];
      if (prof) {
        prof.deploy_ip = host;
        prof.deploy_login = sshLogin;
        if (isManualPorts) {
          prof.deploy_ssh_port = String(sshPort);
          prof.server_peer_port = parseInt(peerPort, 10);
          prof.server_web_port = parseInt(webPort, 10);
        }
      }
    });

    // Prepare CLI arguments
    var args = [
      operation,
      "--host=" + host,
      "--port=" + String(sshPort),
      "--user=" + sshLogin
    ];
    // Non-root users need sudo; the CLI then takes the sudo password from the SSH password.
    if (sshLogin !== "root") args.push("--sudo");

    if (operation === "deploy") {
      args.push("--peer-port=" + String(peerPort));
      args.push("--web-port=" + String(webPort));
      if (dockerInstall) {
        args.push("--docker");
      }
      if (secretUuid) {
        args.push("--secret-path=" + secretUuid);
      }
      if (webLogin) {
        args.push("--web-login=" + webLogin);
      }
    } else if (operation === "undeploy") {
      args.push("--peer-port=" + String(peerPort));
    }

    if (isSshKeysMode && sessionSshKeyPath) {
      args.push("--key=" + sessionSshKeyPath);
      if (sessionSshKeyPassphrase) {
        args.push("--passphrase-env=CSQTT_SSH_PASSPHRASE");
      }
    }

    // Prepare environment variables (restricted strictly to CSQTT_* by bridge and host)
    var env = {};
    if (!isSshKeysMode && sshPassword) {
      env.CSQTT_SSH_PASSWORD = sshPassword;
    }
    if (operation === "deploy") {
      if (mainPassword) {
        env.CSQTT_MAIN_PASSWORD = mainPassword;
      }
      if (webLogin) {
        env.CSQTT_WEB_LOGIN = webLogin;
      }
      if (webPassword) {
        env.CSQTT_WEB_PASSWORD = webPassword;
      }
    }
    if (isSshKeysMode && sessionSshKeyPassphrase) {
      env.CSQTT_SSH_PASSPHRASE = sessionSshKeyPassphrase;
    }

    // Update running state
    isDeploying = true;
    activeOperationType = operation;
    updateRunningUI();

    var opLabel = operation === "deploy" ? "Установка" : "Удаление";
    appendConsoleOutput("\n=== " + opLabel + " CSQTT на " + host + ":" + sshPort + " ===\n");
    window.UI.toast(operation === "deploy" ? "Установка запущена..." : "Удаление запущено...");

    try {
      activeHandle = window.csqtt.runSelf(args, {
        env: env,
        onOutput: appendConsoleOutput
      });
    } catch (err) {
      isDeploying = false;
      activeHandle = null;
      updateRunningUI();
      window.UI.toast("Ошибка запуска: " + (err && err.message ? err.message : String(err)));
      appendConsoleOutput("\n[ERR] Ошибка запуска процесса: " + (err && err.message ? err.message : String(err)) + "\n");
      return;
    }

    activeHandle.done.then(function (res) {
      var code = (res && typeof res.code === "number") ? res.code : 0;
      isDeploying = false;
      activeHandle = null;
      updateRunningUI();

      appendConsoleOutput("\n=== Процесс завершён (код: " + code + ") ===\n");

      if (code === 0) {
        window.UI.toast(operation === "deploy" ? "Установка успешно завершена" : "Удаление успешно завершено");
        // On success, csqtt deploy updates current profile peer and connection password
        window.App.refreshSettings();
      } else {
        window.UI.toast("Процесс завершился с ошибкой (код " + code + ")");
      }
    }).catch(function (err) {
      isDeploying = false;
      activeHandle = null;
      updateRunningUI();
      var msg = err && err.message ? err.message : String(err);
      appendConsoleOutput("\n[ERR] Процесс завершился ошибкой: " + msg + "\n");
      window.UI.toast("Ошибка: " + msg);
    });
  }

  function cancelProcess() {
    if (!isDeploying || !activeHandle) return;
    appendConsoleOutput("\n[LOG] Отправлен запрос на отмену процесса (kill)...\n");
    window.UI.toast("Отмена операции...");
    try {
      activeHandle.kill();
    } catch (err) {
      console.error("kill failed", err);
    }
  }

  // ---------------------------------------------------------------------------
  // Dynamic UI State Updates
  // ---------------------------------------------------------------------------

  function updateRunningUI() {
    if (installBtn) {
      installBtn.style.display = isDeploying ? "none" : "";
    }
    if (uninstallBtn) {
      uninstallBtn.style.display = isDeploying ? "none" : "";
    }
    if (cancelBtn) {
      cancelBtn.style.display = isDeploying ? "" : "none";
    }
    if (runningBanner) {
      runningBanner.style.display = isDeploying ? "" : "none";
      var bannerText = runningBanner.querySelector(".banner-text");
      if (bannerText) {
        bannerText.textContent = activeOperationType === "deploy"
          ? "Выполняется установка CSQTT на сервер…"
          : "Выполняется удаление CSQTT с сервера…";
      }
    }
  }

  function updateAuthStatusBadge() {
    if (!authStatusBadge) return;
    var hasMainPass = is16AlphaNum(sessionMainPassword);
    var p = window.App.profile() || {};
    var hasWebLogin = isAlphaNum(sessionWebLogin || p.deploy_web_login);
    var hasWebPass = isAlphaNum(sessionWebPassword);

    var ready = hasMainPass && hasWebLogin && hasWebPass;
    authStatusBadge.textContent = ready ? "Настроено" : "Требуется настройка";
    authStatusBadge.style.color = ready ? "var(--extended-success, #356A4C)" : "var(--extended-warning, #7B5800)";
    authStatusBadge.style.backgroundColor = ready ? "rgba(74, 222, 128, 0.15)" : "rgba(255, 180, 0, 0.15)";
  }

  function updateSshKeyButtonState() {
    if (!sshKeysBtn) return;
    var filled = sessionSshKeyPath ? 1 : 0;
    sshKeysBtn.textContent = "Ключи SSH " + filled + "/1";
    sshKeysBtn.style.borderColor = filled > 0 ? "var(--md-sys-color-outline-variant)" : "var(--md-sys-color-error)";
  }

  function syncFormFromState(st) {
    if (isDeploying) return; // Do not overwrite user typing while running
    var p = window.App.profile();
    if (!p) return;

    if (hostField && hostField.getValue() !== (p.deploy_ip || "")) {
      hostField.setValue(p.deploy_ip || "");
    }
    if (sshLoginField && sshLoginField.getValue() !== (p.deploy_login || "")) {
      sshLoginField.setValue(p.deploy_login || "");
    }
    if (sshPortField && sshPortField.getValue() !== (p.deploy_ssh_port || String(DEFAULT_SSH_PORT))) {
      sshPortField.setValue(p.deploy_ssh_port || String(DEFAULT_SSH_PORT));
    }
    if (peerPortField && peerPortField.getValue() !== String(p.server_peer_port || DEFAULT_PEER_PORT)) {
      peerPortField.setValue(String(p.server_peer_port || DEFAULT_PEER_PORT));
    }
    if (webPortField && webPortField.getValue() !== String(p.server_web_port || DEFAULT_WEB_PORT)) {
      webPortField.setValue(String(p.server_web_port || DEFAULT_WEB_PORT));
    }

    if (manualPortsContainer) {
      manualPortsContainer.style.display = p.manual_ports_enabled ? "flex" : "none";
    }

    if (sshKeysRowWrapper && sshPasswordRowWrapper) {
      sshKeysRowWrapper.style.display = p.ssh_keys_mode ? "block" : "none";
      sshPasswordRowWrapper.style.display = p.ssh_keys_mode ? "none" : "block";
    }

    if (!p.deploy_secret_path_uuid) {
      var newUuid = generateUuid();
      window.App.saveSettings(function (s) {
        var prof = s.profiles[s.active_profile || 0];
        if (prof) prof.deploy_secret_path_uuid = newUuid;
      });
    }

    if (panelUrlInput) {
      panelUrlInput.value = computePanelUrl(
        p.deploy_ip,
        p.manual_ports_enabled ? p.server_web_port : DEFAULT_WEB_PORT,
        p.deploy_secret_path_uuid
      );
    }

    updateAuthStatusBadge();
    updateSshKeyButtonState();
  }

  // ---------------------------------------------------------------------------
  // Main Screen Renderer
  // ---------------------------------------------------------------------------

  function renderDeployScreen(container) {
    container.innerHTML = "";

    var root = window.UI.h("div", {
      className: "deploy-screen-root",
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "16px",
        paddingBottom: "32px"
      }
    });

    var p = window.App.profile() || {};

    // Generate initial secret UUID if missing
    if (!p.deploy_secret_path_uuid) {
      var initUuid = generateUuid();
      window.App.saveSettings(function (s) {
        var prof = s.profiles[s.active_profile || 0];
        if (prof) prof.deploy_secret_path_uuid = initUuid;
      });
    }

    // Running Banner (visible during deploy/undeploy)
    runningBanner = window.UI.h("div", {
      className: "banner-box",
      style: {
        display: "none",
        backgroundColor: "var(--md-sys-color-primary-container, #E8F0FE)",
        color: "var(--md-sys-color-on-primary-container, #111215)",
        border: "1px solid var(--md-sys-color-primary)",
        justifyContent: "space-between"
      }
    });
    var bannerLeft = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", gap: "10px" }
    });
    var spinner = window.UI.h("div", {
      className: "m3-spinner",
      style: { width: "20px", height: "20px", borderWidth: "2px" }
    });
    var bannerTextEl = window.UI.h("span", {
      className: "banner-text",
      style: { fontSize: "13px", fontWeight: "600" },
      text: "Выполняется установка CSQTT…"
    });
    bannerLeft.appendChild(spinner);
    bannerLeft.appendChild(bannerTextEl);
    runningBanner.appendChild(bannerLeft);

    var bannerCancelBtn = window.UI.button({
      text: "Отмена",
      variant: "outlined",
      className: "m3-button-compact",
      onClick: cancelProcess
    });
    runningBanner.appendChild(bannerCancelBtn);
    root.appendChild(runningBanner);

    // -------------------------------------------------------------------------
    // Main Card: "Деплой и SSH"
    // -------------------------------------------------------------------------
    var mainCard = window.UI.sectionCard({
      gap: "14px"
    });

    var cardTitle = window.UI.h("div", {
      style: {
        fontSize: "16px",
        fontWeight: "600",
        color: "var(--md-sys-color-on-surface)"
      },
      text: "Деплой и SSH"
    });
    mainCard.appendChild(cardTitle);

    // Host Field
    hostField = window.UI.textField({
      label: "IP сервера или домен",
      placeholder: "1.2.3.4",
      value: p.deploy_ip || "",
      onChange: function (val) {
        var clean = val.trim();
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (prof) prof.deploy_ip = clean;
        });
        if (panelUrlInput) {
          panelUrlInput.value = computePanelUrl(
            clean,
            p.manual_ports_enabled ? p.server_web_port : DEFAULT_WEB_PORT,
            p.deploy_secret_path_uuid
          );
        }
      }
    });
    mainCard.appendChild(hostField.el);

    // SSH Login Field
    sshLoginField = window.UI.textField({
      label: "Логин SSH",
      placeholder: "root",
      value: p.deploy_login || "",
      onChange: function (val) {
        var clean = val.trim();
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (prof) prof.deploy_login = clean;
        });
      }
    });
    mainCard.appendChild(sshLoginField.el);

    // SSH Password Field (session-only, never saved to settings)
    sshPasswordRowWrapper = window.UI.h("div", {
      style: { display: p.ssh_keys_mode ? "none" : "block", width: "100%" }
    });
    sshPasswordField = window.UI.textField({
      label: "Пароль SSH",
      placeholder: "пароль пользователя",
      value: sessionSshPassword,
      isPassword: true,
      onInput: function (val) {
        sessionSshPassword = val;
        if (val) sshPasswordField.setError(null);
      }
    });
    sshPasswordRowWrapper.appendChild(sshPasswordField.el);
    mainCard.appendChild(sshPasswordRowWrapper);

    // SSH Keys Button (shown when ssh_keys_mode is enabled)
    sshKeysRowWrapper = window.UI.h("div", {
      style: { display: p.ssh_keys_mode ? "block" : "none", width: "100%" }
    });
    sshKeysBtn = window.UI.button({
      text: "Ключи SSH " + (sessionSshKeyPath ? "1/1" : "0/1"),
      variant: "outlined",
      icon: "Key",
      fullWidth: true,
      onClick: showSshKeysDialog
    });
    sshKeysRowWrapper.appendChild(sshKeysBtn);
    mainCard.appendChild(sshKeysRowWrapper);

    // Switch: "Ключи SSH"
    sshKeysModeRow = window.UI.settingRow({
      title: "Ключи SSH",
      description: "Аутентификация по приватному ключу вместо пароля",
      checked: !!p.ssh_keys_mode,
      onCheckedChange: function (checked) {
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (prof) prof.ssh_keys_mode = checked;
        });
        if (sshKeysRowWrapper && sshPasswordRowWrapper) {
          sshKeysRowWrapper.style.display = checked ? "block" : "none";
          sshPasswordRowWrapper.style.display = checked ? "none" : "block";
        }
      }
    });
    mainCard.appendChild(sshKeysModeRow);

    // Switch: "Ручные порты"
    manualPortsRow = window.UI.settingRow({
      title: "Ручные порты",
      description: "Кастомные порты для SSH, туннеля и web-панели",
      checked: !!p.manual_ports_enabled,
      onCheckedChange: function (checked) {
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (prof) prof.manual_ports_enabled = checked;
        });
        if (manualPortsContainer) {
          manualPortsContainer.style.display = checked ? "flex" : "none";
        }
        if (panelUrlInput) {
          panelUrlInput.value = computePanelUrl(
            p.deploy_ip,
            checked ? p.server_web_port : DEFAULT_WEB_PORT,
            p.deploy_secret_path_uuid
          );
        }
      }
    });
    mainCard.appendChild(manualPortsRow);

    // Manual Ports Sub-Fields (visible when manual_ports_enabled is true)
    manualPortsContainer = window.UI.h("div", {
      style: {
        display: p.manual_ports_enabled ? "flex" : "none",
        gap: "10px",
        width: "100%",
        paddingTop: "2px",
        paddingBottom: "4px"
      }
    });

    sshPortField = window.UI.textField({
      label: "SSH-порт",
      placeholder: "22",
      value: p.deploy_ssh_port || String(DEFAULT_SSH_PORT),
      onChange: function (val) {
        var clean = val.replace(/\D/g, "").slice(0, 5);
        sshPortField.setValue(clean);
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (prof) prof.deploy_ssh_port = clean;
        });
      }
    });
    sshPortField.el.style.flex = "1";
    manualPortsContainer.appendChild(sshPortField.el);

    peerPortField = window.UI.textField({
      label: "Порт туннеля",
      placeholder: "46000",
      value: String(p.server_peer_port || DEFAULT_PEER_PORT),
      onChange: function (val) {
        var clean = val.replace(/\D/g, "").slice(0, 5);
        peerPortField.setValue(clean);
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (prof) prof.server_peer_port = parseInt(clean, 10) || DEFAULT_PEER_PORT;
        });
      }
    });
    peerPortField.el.style.flex = "1";
    manualPortsContainer.appendChild(peerPortField.el);

    webPortField = window.UI.textField({
      label: "Порт WEB-панели",
      placeholder: "46002",
      value: String(p.server_web_port || DEFAULT_WEB_PORT),
      onChange: function (val) {
        var clean = val.replace(/\D/g, "").slice(0, 5);
        webPortField.setValue(clean);
        var num = parseInt(clean, 10) || DEFAULT_WEB_PORT;
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (prof) prof.server_web_port = num;
        });
        if (panelUrlInput) {
          panelUrlInput.value = computePanelUrl(
            p.deploy_ip,
            num,
            p.deploy_secret_path_uuid
          );
        }
      }
    });
    webPortField.el.style.flex = "1";
    manualPortsContainer.appendChild(webPortField.el);

    mainCard.appendChild(manualPortsContainer);

    // Switch: "Установить в Docker"
    dockerInstallRow = window.UI.settingRow({
      title: "Установить в Docker",
      description: "Запуск серверных компонентов в изолированных контейнерах",
      checked: !!p.docker_install,
      infoTitle: "О Docker-установке",
      onInfoClick: function () {
        window.UI.infoDialog(
          "Установка в Docker",
          "При установке в Docker все компоненты CSQTT будут запущены в изолированном контейнере Docker. Убедитесь, что на сервере установлен Docker или установщик автоматически попытается его развернуть."
        );
      },
      onCheckedChange: function (checked) {
        window.App.saveSettings(function (s) {
          var prof = s.profiles[s.active_profile || 0];
          if (prof) prof.docker_install = checked;
        });
      }
    });
    mainCard.appendChild(dockerInstallRow);

    // Secret Path and Panel URL
    var secretPathTitle = window.UI.h("div", {
      style: {
        fontSize: "14px",
        fontWeight: "500",
        color: "var(--md-sys-color-on-surface)",
        paddingTop: "6px"
      },
      text: "Секретный путь и ссылка панели"
    });
    mainCard.appendChild(secretPathTitle);

    var panelUrlRow = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        gap: "8px",
        width: "100%"
      }
    });

    var panelUrlContainer = window.UI.h("div", {
      className: "m3-field-container",
      style: {
        flex: "1",
        cursor: "pointer"
      }
    });
    panelUrlInput = window.UI.h("input", {
      className: "m3-field-input",
      attrs: {
        type: "text",
        readOnly: true
      },
      style: {
        color: "var(--md-sys-color-primary)",
        fontWeight: "500",
        cursor: "pointer"
      }
    });
    panelUrlInput.value = computePanelUrl(
      p.deploy_ip,
      p.manual_ports_enabled ? p.server_web_port : DEFAULT_WEB_PORT,
      p.deploy_secret_path_uuid
    );
    panelUrlContainer.appendChild(panelUrlInput);

    // Clicking the URL field opens the panel in the default browser
    panelUrlContainer.addEventListener("click", function () {
      var currentUrl = panelUrlInput.value;
      if (!p.deploy_ip) {
        window.UI.toast("Сначала укажите IP сервера");
        return;
      }
      window.csqtt.openUrl(currentUrl).catch(function () {
        window.UI.toast("Не удалось открыть веб-панель");
      });
    });

    var regenUuidBtn = window.UI.iconButton({
      icon: "ImportExport",
      title: "Перегенерировать секретный UUID",
      onClick: function (e) {
        e.stopPropagation();
        window.UI.confirm({
          title: "Перегенерация пути панели",
          message: "После перегенерации старый путь к панели будет изменен. Перегенерировать UUID сейчас?",
          confirmText: "Перегенерировать",
          cancelText: "Отмена"
        }).then(function (ok) {
          if (!ok) return;
          var newUuid = generateUuid();
          window.App.saveSettings(function (s) {
            var prof = s.profiles[s.active_profile || 0];
            if (prof) prof.deploy_secret_path_uuid = newUuid;
          }).then(function () {
            panelUrlInput.value = computePanelUrl(
              p.deploy_ip,
              p.manual_ports_enabled ? p.server_web_port : DEFAULT_WEB_PORT,
              newUuid
            );
            window.UI.toast("Секретный путь обновлен");
          });
        });
      }
    });

    panelUrlRow.appendChild(panelUrlContainer);
    panelUrlRow.appendChild(regenUuidBtn);
    mainCard.appendChild(panelUrlRow);

    root.appendChild(mainCard);

    // -------------------------------------------------------------------------
    // Authorization Settings Row Card
    // -------------------------------------------------------------------------
    var authCard = window.UI.sectionCard();
    var authRow = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        width: "100%",
        cursor: "pointer"
      }
    });
    authRow.addEventListener("click", showAuthorizationDialog);

    var authLeft = window.UI.h("div", {
      style: { display: "flex", flexDirection: "column", gap: "2px" }
    });
    var authTitle = window.UI.h("div", {
      style: { fontSize: "15px", fontWeight: "600", color: "var(--md-sys-color-on-surface)" },
      text: "Авторизация"
    });
    var authDesc = window.UI.h("div", {
      style: { fontSize: "12px", color: "var(--md-sys-color-on-surface-variant)" },
      text: "Пароль туннеля (16 символов) и доступ к web-панели"
    });
    authLeft.appendChild(authTitle);
    authLeft.appendChild(authDesc);

    var authRight = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", gap: "8px" }
    });
    authStatusBadge = window.UI.h("span", {
      style: {
        fontSize: "11px",
        fontWeight: "700",
        padding: "3px 8px",
        borderRadius: "var(--shape-pill)",
        letterSpacing: "0.4px"
      }
    });
    updateAuthStatusBadge();
    authRight.appendChild(authStatusBadge);

    var authEditBtn = window.UI.button({
      text: "Изменить",
      variant: "tonal",
      icon: "Key",
      onClick: function (e) {
        e.stopPropagation();
        showAuthorizationDialog();
      }
    });
    authRight.appendChild(authEditBtn);

    authRow.appendChild(authLeft);
    authRow.appendChild(authRight);
    authCard.appendChild(authRow);
    root.appendChild(authCard);

    // -------------------------------------------------------------------------
    // Primary Action Buttons (Install, Uninstall, Control Panel, Cancel)
    // -------------------------------------------------------------------------
    var actionsContainer = window.UI.h("div", {
      style: {
        display: "flex",
        flexDirection: "column",
        gap: "10px",
        width: "100%"
      }
    });

    var buttonRow = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        gap: "10px",
        width: "100%"
      }
    });

    installBtn = window.UI.button({
      text: "Установить",
      variant: "filled",
      className: "m3-button-hero",
      onClick: function () {
        startProcess("deploy");
      }
    });
    installBtn.style.flex = "1";
    installBtn.style.height = "52px";
    buttonRow.appendChild(installBtn);

    uninstallBtn = window.UI.button({
      text: "Удалить",
      variant: "outlined",
      icon: "Delete",
      onClick: function () {
        showUninstallConfirmDialog(function () {
          startProcess("undeploy");
        });
      }
    });
    uninstallBtn.style.height = "52px";
    buttonRow.appendChild(uninstallBtn);

    var openPanelBtn = window.UI.button({
      text: "Панель управления",
      variant: "tonal",
      onClick: function () {
        if (!p.deploy_ip) {
          window.UI.toast("Сначала укажите IP сервера");
          return;
        }
        var url = computePanelUrl(
          p.deploy_ip,
          p.manual_ports_enabled ? p.server_web_port : DEFAULT_WEB_PORT,
          p.deploy_secret_path_uuid
        );
        window.csqtt.openUrl(url).catch(function () {
          window.UI.toast("Не удалось открыть панель управления");
        });
      }
    });
    openPanelBtn.style.height = "52px";
    buttonRow.appendChild(openPanelBtn);

    cancelBtn = window.UI.button({
      text: "Отмена процесса",
      variant: "filled",
      onClick: cancelProcess
    });
    cancelBtn.style.flex = "1";
    cancelBtn.style.height = "52px";
    cancelBtn.style.backgroundColor = "var(--md-sys-color-error, #BA1A1A)";
    cancelBtn.style.color = "var(--md-sys-color-on-error, #FFFFFF)";
    cancelBtn.style.display = "none";
    buttonRow.appendChild(cancelBtn);

    actionsContainer.appendChild(buttonRow);
    root.appendChild(actionsContainer);

    // -------------------------------------------------------------------------
    // Monospace Console Card (Scrollable, 2000 lines, textContent)
    // -------------------------------------------------------------------------
    var consoleCard = window.UI.h("div", {
      className: "logs-terminal-card",
      style: {
        minHeight: "260px",
        maxHeight: "380px",
        display: "flex",
        flexDirection: "column",
        position: "relative",
        borderRadius: "var(--shape-card, 34px)",
        backgroundColor: "var(--terminal-bg, #15171B)",
        color: "var(--terminal-text, #DDE1E8)",
        overflow: "hidden",
        boxShadow: "0 4px 14px rgba(0, 0, 0, 0.3)"
      }
    });

    // Console Header Toolbar
    var consoleHeader = window.UI.h("div", {
      style: {
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "10px 16px",
        borderBottom: "1px solid rgba(255, 255, 255, 0.08)",
        backgroundColor: "rgba(255, 255, 255, 0.02)"
      }
    });

    var consoleHeaderLeft = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", gap: "8px" }
    });
    consoleHeaderLeft.appendChild(window.Icons.el("Terminal", 18, { fill: "var(--terminal-muted, #9DA3AE)" }));
    var consoleHeaderTitle = window.UI.h("span", {
      style: { fontSize: "13px", fontWeight: "600", color: "var(--terminal-text, #DDE1E8)" },
      text: "Консоль деплоя"
    });
    consoleCountBadgeEl = window.UI.h("span", {
      style: {
        fontSize: "11px",
        color: "var(--terminal-muted, #9DA3AE)",
        fontFamily: "var(--font-mono)",
        marginLeft: "4px"
      },
      text: consoleLines.length + " строк"
    });
    consoleHeaderLeft.appendChild(consoleHeaderTitle);
    consoleHeaderLeft.appendChild(consoleCountBadgeEl);

    var consoleHeaderRight = window.UI.h("div", {
      style: { display: "flex", alignItems: "center", gap: "6px" }
    });
    var copyBtn = window.UI.iconButton({
      icon: "ContentCopy",
      title: "Копировать вывод",
      size: 16,
      onClick: copyConsole
    });
    var clearBtn = window.UI.iconButton({
      icon: "Delete",
      title: "Очистить консоль",
      size: 16,
      onClick: clearConsole
    });
    consoleHeaderRight.appendChild(copyBtn);
    consoleHeaderRight.appendChild(clearBtn);

    consoleHeader.appendChild(consoleHeaderLeft);
    consoleHeader.appendChild(consoleHeaderRight);
    consoleCard.appendChild(consoleHeader);

    // Console Viewport (Monospace pre/div, textContent only)
    consoleViewportEl = window.UI.h("div", {
      style: {
        flex: "1",
        overflowY: "auto",
        overflowX: "hidden",
        padding: "12px 14px",
        fontFamily: "var(--font-mono, Consolas, monospace)",
        fontSize: "12px",
        lineHeight: "18px",
        whiteSpace: "pre-wrap",
        wordBreak: "break-all",
        userSelect: "text",
        webkitUserSelect: "text"
      }
    });
    consoleCard.appendChild(consoleViewportEl);
    root.appendChild(consoleCard);

    container.appendChild(root);

    // Initial render of existing console output
    renderConsoleViewport();
    updateRunningUI();

    // Subscribe to state changes from settings service
    window.App.onState(syncFormFromState);
  }

  // ---------------------------------------------------------------------------
  // Register Screen with App Navigation
  // ---------------------------------------------------------------------------
  window.App.registerScreen({
    id: "deploy",
    title: "Деплой",
    icon: "Cloud",
    order: 2,
    render: renderDeployScreen
  });

  // Public screen API
  window.DeployScreen = {
    startProcess: startProcess,
    cancelProcess: cancelProcess,
    showAuthorizationDialog: showAuthorizationDialog,
    showSshKeysDialog: showSshKeysDialog,
    showUninstallConfirmDialog: showUninstallConfirmDialog,
    getConsoleLines: function () { return consoleLines.slice(); },
    clearConsole: clearConsole
  };
})();
