/**
 * Logs Screen for csqtt-win
 *
 * Implements LogsTab.kt with 60 fps virtualized list rendering,
 * cap at 5000 lines, batched requestAnimationFrame updates,
 * LogLine.kt coloring rules, ANSI escape parsing, duplicate counter,
 * auto-scroll with jump-to-bottom button, copy all and clear buttons.
 */

(function () {
  "use strict";

  var ROW_HEIGHT = 26;
  var OVERSCAN = 6;
  var MAX_LOGS = 5000;

  var allLogs = [];
  var pendingQueue = [];
  var isRafScheduled = false;
  var isAtBottom = true;
  var viewportEl = null;
  var itemsContainerEl = null;
  var scrollHeightEl = null;
  var scrollBottomBtn = null;
  var emptyStateEl = null;

  function parseLogLine(rawMessage) {
    var trimmed = (rawMessage || "").trim();
    var tag = "LOG";
    var body = trimmed;

    if (trimmed.charAt(0) === "[") {
      var closeIdx = trimmed.indexOf("]");
      if (closeIdx > 0 && closeIdx <= 24) {
        tag = trimmed.slice(1, closeIdx).trim();
        body = trimmed.slice(closeIdx + 1).trim();
      }
    }

    var upperTag = tag.toUpperCase();
    var isError = upperTag.indexOf("ERR") !== -1 ||
                  upperTag.indexOf("ОШИБКА") !== -1 ||
                  upperTag.indexOf("ФАТАЛ") !== -1;

    var isGreen = !isError && (
      upperTag.indexOf("OK") !== -1 ||
      upperTag.indexOf("NET") !== -1 ||
      upperTag.indexOf("СЕТЬ") !== -1 ||
      upperTag.indexOf("WRAP") !== -1 ||
      upperTag.indexOf("ВОРКЕР") !== -1
    );

    var displayTag = isGreen ? "OK" : tag;
    var tagType = isError ? "error" : (isGreen ? "green" : "normal");

    return {
      raw: rawMessage,
      tag: displayTag,
      tagType: tagType,
      body: body,
      isError: isError,
      isGreen: isGreen,
      count: 1
    };
  }

  function parseAnsiToNodes(text, defaultColor, greenColor, redColor) {
    var ansiRegex = /\u001B\[([0-9;]+)m/g;
    var fragments = [];
    var lastIndex = 0;
    var currentColor = defaultColor;
    var match;

    while ((match = ansiRegex.exec(text)) !== null) {
      if (match.index > lastIndex) {
        fragments.push({
          text: text.substring(lastIndex, match.index),
          color: currentColor
        });
      }
      var code = match[1];
      if (code === "32" || code === "92") currentColor = greenColor;
      else if (code === "31" || code === "91") currentColor = redColor;
      else if (code === "0") currentColor = defaultColor;
      lastIndex = ansiRegex.lastIndex;
    }

    if (lastIndex < text.length) {
      fragments.push({
        text: text.substring(lastIndex),
        color: currentColor
      });
    }

    if (fragments.length === 0) {
      fragments.push({ text: text, color: defaultColor });
    }

    var spanNodes = [];
    fragments.forEach(function (frag) {
      var span = document.createElement("span");
      span.textContent = frag.text;
      if (frag.color) span.style.color = frag.color;
      spanNodes.push(span);
    });
    return spanNodes;
  }

  function addLogLine(raw) {
    if (!raw) return;
    var parsed = parseLogLine(raw);

    // Check duplicate of previous line
    if (allLogs.length > 0) {
      var prev = allLogs[allLogs.length - 1];
      if (prev.raw === parsed.raw) {
        prev.count++;
        scheduleRender();
        return;
      }
    }

    allLogs.push(parsed);
    if (allLogs.length > MAX_LOGS) {
      allLogs.splice(0, allLogs.length - MAX_LOGS);
    }
    scheduleRender();
  }

  function addLogBatch(lines) {
    if (!lines || lines.length === 0) return;
    for (var i = 0; i < lines.length; i++) {
      var parsed = parseLogLine(lines[i]);
      if (allLogs.length > 0 && allLogs[allLogs.length - 1].raw === parsed.raw) {
        allLogs[allLogs.length - 1].count++;
      } else {
        allLogs.push(parsed);
      }
    }
    if (allLogs.length > MAX_LOGS) {
      allLogs.splice(0, allLogs.length - MAX_LOGS);
    }
    scheduleRender();
  }

  function scheduleRender() {
    if (isRafScheduled) return;
    isRafScheduled = true;
    requestAnimationFrame(function () {
      isRafScheduled = false;
      renderVirtual();
    });
  }

  function renderVirtual() {
    if (!viewportEl || !itemsContainerEl || !scrollHeightEl) return;

    var total = allLogs.length;
    scrollHeightEl.style.height = (total * ROW_HEIGHT) + "px";

    if (total === 0) {
      if (emptyStateEl) emptyStateEl.style.display = "flex";
      itemsContainerEl.innerHTML = "";
      return;
    }
    if (emptyStateEl) emptyStateEl.style.display = "none";

    var viewportHeight = viewportEl.clientHeight || 400;
    var scrollTop = viewportEl.scrollTop;

    // Check bottom threshold
    var maxScroll = Math.max(0, (total * ROW_HEIGHT) - viewportHeight);
    isAtBottom = (maxScroll - scrollTop) <= 40;

    if (scrollBottomBtn) {
      if (!isAtBottom && total > 10) {
        scrollBottomBtn.classList.add("visible");
      } else {
        scrollBottomBtn.classList.remove("visible");
      }
    }

    var startIndex = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
    var endIndex = Math.min(total, Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN);

    itemsContainerEl.innerHTML = "";

    var docFrag = document.createDocumentFragment();
    for (var i = startIndex; i < endIndex; i++) {
      var item = allLogs[i];
      var row = document.createElement("div");
      row.className = "log-row";
      row.style.position = "absolute";
      row.style.top = (i * ROW_HEIGHT) + "px";
      row.style.left = "14px";
      row.style.right = "14px";
      row.style.height = ROW_HEIGHT + "px";

      var badge = document.createElement("span");
      badge.className = "log-tag-badge " + item.tagType;
      badge.textContent = item.tag;
      row.appendChild(badge);

      var bodySpan = document.createElement("span");
      bodySpan.className = "log-body " + item.tagType;
      bodySpan.style.whiteSpace = "nowrap";
      bodySpan.style.overflow = "hidden";
      bodySpan.style.textOverflow = "ellipsis";
      bodySpan.title = item.raw;

      var colorMap = {
        error: "#FFB4AB",
        green: "#9BD4AF",
        normal: "#DDE1E8"
      };
      var nodes = parseAnsiToNodes(item.body, colorMap[item.tagType], "#9BD4AF", "#FFB4AB");
      nodes.forEach(function (n) { bodySpan.appendChild(n); });
      row.appendChild(bodySpan);

      if (item.count > 1) {
        var countBadge = document.createElement("span");
        countBadge.className = "log-count-badge";
        countBadge.textContent = "x" + item.count;
        row.appendChild(countBadge);
      }

      docFrag.appendChild(row);
    }
    itemsContainerEl.appendChild(docFrag);

    if (isAtBottom) {
      viewportEl.scrollTop = maxScroll;
    }
  }

  function renderLogsScreen(container) {
    container.innerHTML = "";

    // Header card with Active Logging switch
    var headerCard = window.UI.sectionCard({ padding: "8px 16px" });
    var logSwitch = window.UI.settingRow({
      title: "Активное логирование",
      checked: true,
      onCheckedChange: function (checked) {
        window.App.saveSettings(function (s) {
          s.logging_enabled = checked;
        });
      }
    });
    headerCard.appendChild(logSwitch);
    container.appendChild(headerCard);

    // Terminal container card
    var terminalCard = window.UI.h("div", { className: "logs-terminal-card" });

    viewportEl = window.UI.h("div", { className: "logs-viewport" });
    scrollHeightEl = window.UI.h("div", { className: "logs-virtual-scroll-height" });
    itemsContainerEl = window.UI.h("div", { className: "logs-items-container" });

    viewportEl.appendChild(scrollHeightEl);
    viewportEl.appendChild(itemsContainerEl);

    // Empty state
    emptyStateEl = window.UI.emptyState({
      title: "Событий пока нет",
      description: "Новые записи появятся при работе туннеля",
      icon: "TerminalOutlined"
    });
    emptyStateEl.style.height = "100%";
    emptyStateEl.style.display = "flex";
    viewportEl.appendChild(emptyStateEl);

    viewportEl.addEventListener("scroll", function () {
      var viewportHeight = viewportEl.clientHeight || 400;
      var total = allLogs.length;
      var maxScroll = Math.max(0, (total * ROW_HEIGHT) - viewportHeight);
      isAtBottom = (maxScroll - viewportEl.scrollTop) <= 40;
      scheduleRender();
    });

    terminalCard.appendChild(viewportEl);

    // Floating action toolbar
    var toolbar = window.UI.h("div", { className: "logs-toolbar" });

    // Scroll to bottom button
    scrollBottomBtn = window.UI.iconButton({
      icon: "ArrowDownward",
      title: "Вниз",
      size: 20,
      className: "logs-scroll-bottom-btn logs-action-btn",
      onClick: function () {
        var viewportHeight = viewportEl.clientHeight || 400;
        var maxScroll = Math.max(0, (allLogs.length * ROW_HEIGHT) - viewportHeight);
        viewportEl.scrollTop = maxScroll;
        isAtBottom = true;
        scheduleRender();
      }
    });
    toolbar.appendChild(scrollBottomBtn);

    // Copy all button
    var copyBtn = window.UI.iconButton({
      icon: "ContentCopy",
      title: "Копировать все логи",
      size: 20,
      className: "logs-action-btn",
      onClick: function () {
        if (allLogs.length === 0) return;
        var text = allLogs.map(function (item) {
          return item.raw + (item.count > 1 ? " (x" + item.count + ")" : "");
        }).join("\n");
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(text).then(function () {
            window.UI.toast("Логи скопированы");
          });
        } else {
          window.UI.infoDialog("Логи", text);
        }
      }
    });
    toolbar.appendChild(copyBtn);

    // Clear logs button
    var clearBtn = window.UI.iconButton({
      icon: "DeleteOutlined",
      title: "Очистить логи",
      size: 20,
      className: "logs-action-btn",
      onClick: function () {
        allLogs = [];
        window.csqtt.ipc({ cmd: "logs_clear" }).catch(function () {});
        scheduleRender();
        window.UI.toast("Логи очищены");
      }
    });
    toolbar.appendChild(clearBtn);

    terminalCard.appendChild(toolbar);
    container.appendChild(terminalCard);

    // Sync settings state
    window.App.onState(function (st) {
      if (st.settings && logSwitch.setChecked) {
        logSwitch.setChecked(!!st.settings.logging_enabled);
      }
    });

    // Fetch initial logs tail
    window.csqtt.logsTail(500).then(function (lines) {
      if (lines && lines.length > 0) {
        addLogBatch(lines);
      }
    }).catch(function (err) {
      console.warn("logsTail error:", err);
    });

    scheduleRender();
  }

  // Subscribe to live log events
  window.csqtt.on("log", function (data) {
    if (data && data.line) {
      addLogLine(data.line);
    }
  });

  // Register Logs Screen
  window.App.registerScreen({
    id: "logs",
    title: "Логи",
    icon: "Terminal",
    order: 3,
    render: renderLogsScreen,
    onShow: function () {
      isAtBottom = true;
      scheduleRender();
    }
  });
})();
