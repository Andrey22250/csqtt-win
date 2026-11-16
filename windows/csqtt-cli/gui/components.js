/**
 * UI Component Library for csqtt-win (Material 3)
 *
 * All functions return DOM Elements (unless otherwise noted).
 *
 * PUBLIC API:
 *
 * UI.h(tag, props, ...children) -> HTMLElement
 *   Creates a DOM element with given attributes, styles, event listeners, and children.
 *   - tag: string (e.g. 'div', 'button', 'span')
 *   - props: object | null:
 *       className?: string
 *       id?: string
 *       text?: string (set via textContent)
 *       style?: { [cssProp: string]: string }
 *       attrs?: { [attr: string]: string | number | boolean }
 *       on?: { [event: string]: (e: Event) => void }
 *   - children: variable arguments or arrays of (HTMLElement | string | number | null | undefined)
 *
 * UI.sectionCard(optionsOrChildren) -> HTMLElement
 *   Returns an AppSectionCard container element (.section-card).
 *   - optionsOrChildren: { padding?: string, gap?: string, children?: Array } or Array/HTMLElement
 *
 * UI.settingRow({ title, description, checked, enabled, onCheckedChange, onInfoClick, infoTitle }) -> HTMLElement
 *   Returns a setting row with title, description, and switch (.setting-row).
 *   - title: string
 *   - description?: string
 *   - checked: boolean
 *   - enabled?: boolean (default true)
 *   - onCheckedChange?: (checked: boolean) => void
 *   - onInfoClick?: () => void
 *   - infoTitle?: string
 *
 * UI.switchRow(options) -> HTMLElement
 *   Alias to UI.settingRow.
 *
 * UI.switchWidget({ checked, enabled, onChange }) -> HTMLElement
 *   Returns a standalone Material 3 switch element (.m3-switch).
 *
 * UI.textField({ label, placeholder, value, type, isPassword, error, enabled, singleLine, onInput, onChange, trailing })
 *   -> { el: HTMLElement, input: HTMLInputElement, getValue(): string, setValue(val: string), setError(err: string|null) }
 *   Returns a text field with floating label, error state, and password visibility toggle.
 *
 * UI.dropdown({ title, selectedKey, options, enabled, onSelected, onInfo, onTitleInfo, indicatorProvider }) -> HTMLElement
 *   Returns a Material 3 dropdown selector with optional indicator bar.
 *   - options: Array of { key: string, label: string }
 *   - onSelected?: (key: string) => void
 *   - onInfo?: (key: string) => void
 *   - onTitleInfo?: () => void
 *   - indicatorProvider?: (key: string) => { progress: number, color: string } | null
 *
 * UI.segmented({ options, selected, onSelected, enabled }) -> HTMLElement
 *   Returns a segmented control (horizontal pill options).
 *   - options: Array of { key: any, label: string }
 *   - onSelected: (key: any) => void
 *
 * UI.slider({ min, max, step, value, label, valueFormatter, enabled, onValueChange, onInfo }) -> HTMLElement
 *   Returns a slider with value text label and optional info icon button.
 *   - valueFormatter?: (val: number) => string
 *   - onValueChange: (val: number) => void
 *
 * UI.button({ text, variant, icon, onClick, enabled, fullWidth, className }) -> HTMLElement
 *   Returns a Material 3 button (.m3-button).
 *   - variant: "filled" | "tonal" | "outlined" | "text" (default "filled")
 *
 * UI.iconButton({ icon, title, onClick, enabled, className, size }) -> HTMLElement
 *   Returns an icon button (.m3-icon-button).
 *   - icon: string (icon name from Icons) or SVG Element
 *
 * UI.chip({ label, selected, enabled, onClick }) -> HTMLElement
 *   Returns an M3 chip button (.m3-chip).
 *
 * UI.divider() -> HTMLElement
 *   Returns a horizontal divider line (.m3-divider).
 *
 * UI.dialog({ title, content, actions, onClose }) -> { el: HTMLElement, close() }
 *   Displays a modal dialog over #dialog-root with backdrop scrim.
 *   - actions: Array of { text: string, variant?: string, onClick: (dialog) => void }
 *
 * UI.confirm({ title, message, confirmText, cancelText }) -> Promise<boolean>
 *   Shows a confirmation dialog and resolves with true (confirmed) or false (cancelled).
 *
 * UI.infoDialog(title, bodyTextOrEl) -> { el: HTMLElement, close() }
 *   Shows an informational dialog with a "Понятно" button.
 *
 * UI.toast(text, durationMs) -> void
 *   Displays a floating toast notification in #toast-root (default 2500ms).
 *
 * UI.loadingState(label) -> HTMLElement
 *   Returns a loading spinner container with text label.
 *
 * UI.emptyState({ title, description, icon }) -> HTMLElement
 *   Returns an empty state container with icon, title, and optional description.
 *
 * UI.banner({ text, containerColor, contentColor, icon, action }) -> HTMLElement
 *   Returns an inline alert banner.
 */

(function () {
  "use strict";

  function h(tag, props) {
    var el = document.createElement(tag);
    var children = Array.prototype.slice.call(arguments, 2);
    if (props) {
      if (props.className) el.className = props.className;
      if (props.id) el.id = props.id;
      if (props.text !== undefined && props.text !== null) el.textContent = String(props.text);
      if (props.style) {
        for (var s in props.style) {
          if (Object.prototype.hasOwnProperty.call(props.style, s)) el.style[s] = props.style[s];
        }
      }
      if (props.attrs) {
        for (var a in props.attrs) {
          if (Object.prototype.hasOwnProperty.call(props.attrs, a)) el.setAttribute(a, props.attrs[a]);
        }
      }
      if (props.on) {
        for (var ev in props.on) {
          if (Object.prototype.hasOwnProperty.call(props.on, ev)) el.addEventListener(ev, props.on[ev]);
        }
      }
    }
    function append(c) {
      if (c === null || c === undefined || c === false) return;
      if (Array.isArray(c)) {
        c.forEach(append);
      } else if (c instanceof Node) {
        el.appendChild(c);
      } else {
        el.appendChild(document.createTextNode(String(c)));
      }
    }
    children.forEach(append);
    return el;
  }

  function sectionCard(optionsOrChildren) {
    var card = h("div", { className: "section-card" });
    if (!optionsOrChildren) return card;

    if (Array.isArray(optionsOrChildren)) {
      optionsOrChildren.forEach(function (child) {
        if (child instanceof Node) card.appendChild(child);
      });
      return card;
    }
    if (optionsOrChildren instanceof Node) {
      card.appendChild(optionsOrChildren);
      return card;
    }
    if (typeof optionsOrChildren === "object") {
      if (optionsOrChildren.padding) card.style.padding = optionsOrChildren.padding;
      if (optionsOrChildren.gap) card.style.gap = optionsOrChildren.gap;
      if (optionsOrChildren.className) card.className += " " + optionsOrChildren.className;
      if (Array.isArray(optionsOrChildren.children)) {
        optionsOrChildren.children.forEach(function (child) {
          if (child instanceof Node) card.appendChild(child);
        });
      }
    }
    return card;
  }

  function switchWidget(opts) {
    opts = opts || {};
    var isChecked = !!opts.checked;
    var isEnabled = opts.enabled !== false;

    var switchEl = h("div", {
      className: "m3-switch" + (isChecked ? " checked" : "") + (!isEnabled ? " disabled" : ""),
      attrs: { role: "switch", "aria-checked": isChecked ? "true" : "false" }
    });
    var thumb = h("div", { className: "m3-switch-thumb" });
    switchEl.appendChild(thumb);

    if (isEnabled && opts.onChange) {
      switchEl.addEventListener("click", function (e) {
        e.stopPropagation();
        isChecked = !isChecked;
        if (isChecked) switchEl.classList.add("checked");
        else switchEl.classList.remove("checked");
        switchEl.setAttribute("aria-checked", isChecked ? "true" : "false");
        opts.onChange(isChecked);
      });
    }

    switchEl.setChecked = function (val) {
      isChecked = !!val;
      if (isChecked) switchEl.classList.add("checked");
      else switchEl.classList.remove("checked");
      switchEl.setAttribute("aria-checked", isChecked ? "true" : "false");
    };
    switchEl.isChecked = function () {
      return isChecked;
    };
    return switchEl;
  }

  function settingRow(opts) {
    opts = opts || {};
    var isEnabled = opts.enabled !== false;
    var row = h("div", {
      className: "setting-row" + (!isEnabled ? " disabled" : ""),
      attrs: { role: "button" }
    });

    var textWrap = h("div", { className: "setting-row-text" });
    var headerRow = h("div", { className: "setting-row-header" });
    var titleEl = h("span", { className: "setting-row-title", text: opts.title || "" });
    headerRow.appendChild(titleEl);

    if (opts.onInfoClick) {
      var infoBtn = iconButton({
        icon: "HelpOutline",
        title: opts.infoTitle || "Подробнее",
        size: 16,
        onClick: function (e) {
          e.stopPropagation();
          opts.onInfoClick();
        }
      });
      infoBtn.style.width = "24px";
      infoBtn.style.height = "24px";
      headerRow.appendChild(infoBtn);
    }
    textWrap.appendChild(headerRow);

    if (opts.description) {
      var descEl = h("span", { className: "setting-row-desc", text: opts.description });
      textWrap.appendChild(descEl);
    }
    row.appendChild(textWrap);

    var sw = switchWidget({
      checked: opts.checked,
      enabled: isEnabled,
      onChange: opts.onCheckedChange
    });
    row.appendChild(sw);

    if (isEnabled && opts.onCheckedChange) {
      row.addEventListener("click", function () {
        var next = !sw.isChecked();
        sw.setChecked(next);
        opts.onCheckedChange(next);
      });
    }

    row.setChecked = function (val) {
      sw.setChecked(val);
    };
    return row;
  }

  function textField(opts) {
    opts = opts || {};
    var field = h("div", { className: "m3-field" + (opts.error ? " has-error" : "") });
    if (opts.label) {
      var label = h("label", { className: "m3-field-label", text: opts.label });
      field.appendChild(label);
    }

    var container = h("div", { className: "m3-field-container" });
    var isPass = !!opts.isPassword;
    var inputType = isPass ? "password" : (opts.type || "text");

    var input = h("input", {
      className: "m3-field-input",
      attrs: {
        type: inputType,
        placeholder: opts.placeholder || "",
        value: opts.value !== undefined ? String(opts.value) : "",
        autocomplete: "off",
        spellcheck: "false"
      }
    });
    if (opts.enabled === false) input.disabled = true;

    if (opts.onInput) {
      input.addEventListener("input", function (e) {
        opts.onInput(e.target.value);
      });
    }
    if (opts.onChange) {
      input.addEventListener("change", function (e) {
        opts.onChange(e.target.value);
      });
    }
    container.appendChild(input);

    var trailingWrap = h("div", { className: "m3-field-trailing" });

    if (isPass) {
      var visible = false;
      var eyeBtn = iconButton({
        icon: "Visibility",
        title: "Показать пароль",
        size: 18,
        onClick: function () {
          visible = !visible;
          input.type = visible ? "text" : "password";
          eyeBtn.innerHTML = "";
          eyeBtn.appendChild(window.Icons.el(visible ? "VisibilityOff" : "Visibility", 18));
        }
      });
      trailingWrap.appendChild(eyeBtn);
    }

    if (opts.trailing instanceof Node) {
      trailingWrap.appendChild(opts.trailing);
    }
    if (trailingWrap.children.length > 0) {
      container.appendChild(trailingWrap);
    }
    field.appendChild(container);

    var errorEl = h("div", {
      className: "m3-field-error",
      text: opts.error || "",
      style: { display: opts.error ? "block" : "none" }
    });
    field.appendChild(errorEl);

    return {
      el: field,
      input: input,
      getValue: function () { return input.value; },
      setValue: function (val) { input.value = val || ""; },
      setError: function (msg) {
        if (msg) {
          field.classList.add("has-error");
          errorEl.textContent = msg;
          errorEl.style.display = "block";
        } else {
          field.classList.remove("has-error");
          errorEl.textContent = "";
          errorEl.style.display = "none";
        }
      }
    };
  }

  function dropdown(opts) {
    opts = opts || {};
    var wrap = h("div", { className: "m3-dropdown" });

    if (opts.title) {
      var titleRow = h("div", { className: "m3-dropdown-title-row" });
      var titleText = h("span", {
        className: "label-medium text-muted",
        text: opts.title
      });
      titleRow.appendChild(titleText);

      if (opts.onTitleInfo) {
        var titleInfoBtn = iconButton({
          icon: "HelpOutline",
          size: 15,
          onClick: opts.onTitleInfo
        });
        titleInfoBtn.style.width = "22px";
        titleInfoBtn.style.height = "22px";
        titleRow.appendChild(titleInfoBtn);
      }
      wrap.appendChild(titleRow);
    }

    var selectedKey = opts.selectedKey;
    var options = opts.options || [];

    var trigger = h("div", {
      className: "m3-dropdown-trigger",
      attrs: { role: "button" }
    });

    var valueArea = h("div", { className: "m3-dropdown-value" });

    var indicator = null;
    if (opts.indicatorProvider) {
      indicator = h("div", { className: "indicator-bar" });
      var fill = h("div", { className: "indicator-fill" });
      indicator.appendChild(fill);
      valueArea.appendChild(indicator);
    }

    var labelEl = h("span", { className: "body-medium" });
    valueArea.appendChild(labelEl);
    trigger.appendChild(valueArea);

    var rightArea = h("div", { style: { display: "flex", alignItems: "center", gap: "4px" } });
    if (opts.onInfo) {
      var itemInfoBtn = iconButton({
        icon: "HelpOutline",
        size: 15,
        onClick: function (e) {
          e.stopPropagation();
          opts.onInfo(selectedKey);
        }
      });
      itemInfoBtn.style.width = "22px";
      itemInfoBtn.style.height = "22px";
      rightArea.appendChild(itemInfoBtn);
    }

    var chevron = window.Icons.el("ChevronDown", 20);
    rightArea.appendChild(chevron);
    trigger.appendChild(rightArea);

    function updateDisplay() {
      var found = options.filter(function (o) { return o.key === selectedKey; })[0];
      labelEl.textContent = found ? found.label : selectedKey;

      if (indicator && opts.indicatorProvider) {
        var info = opts.indicatorProvider(selectedKey);
        if (info && indicator.firstChild) {
          indicator.style.display = "block";
          indicator.firstChild.style.width = Math.round(info.progress * 100) + "%";
          indicator.firstChild.style.backgroundColor = info.color || "#1A73E8";
        } else {
          indicator.style.display = "none";
        }
      }
    }
    updateDisplay();

    trigger.addEventListener("click", function () {
      if (opts.enabled === false) return;
      var menuContent = h("div", { style: { display: "flex", flexDirection: "column", gap: "4px" } });
      var dlg = dialog({
        title: opts.title || "Выбор",
        content: menuContent
      });

      options.forEach(function (opt) {
        var isSel = opt.key === selectedKey;
        var itemBtn = h("button", {
          className: "m3-button outlined" + (isSel ? " filled" : ""),
          style: { justifyContent: "space-between", height: "46px" },
          on: {
            click: function () {
              selectedKey = opt.key;
              updateDisplay();
              dlg.close();
              if (opts.onSelected) opts.onSelected(selectedKey);
            }
          }
        });
        itemBtn.appendChild(h("span", { text: opt.label }));
        if (isSel) {
          itemBtn.appendChild(window.Icons.el("Check", 18));
        }
        menuContent.appendChild(itemBtn);
      });
    });

    wrap.appendChild(trigger);
    wrap.setSelectedKey = function (k) {
      selectedKey = k;
      updateDisplay();
    };
    return wrap;
  }

  function segmented(opts) {
    opts = opts || {};
    var wrap = h("div", { className: "segmented-control" });
    var selected = opts.selected;

    (opts.options || []).forEach(function (opt) {
      var isSel = opt.key === selected;
      var btn = h("button", {
        className: "segmented-option" + (isSel ? " selected" : ""),
        attrs: { type: "button" },
        on: {
          click: function () {
            if (opts.enabled === false) return;
            selected = opt.key;
            var buttons = wrap.querySelectorAll(".segmented-option");
            for (var i = 0; i < buttons.length; i++) {
              buttons[i].classList.remove("selected");
            }
            btn.classList.add("selected");
            if (opts.onSelected) opts.onSelected(selected);
          }
        }
      });
      btn.appendChild(h("span", { text: opt.label }));
      wrap.appendChild(btn);
    });

    wrap.setSelected = function (k) {
      selected = k;
      var buttons = wrap.querySelectorAll(".segmented-option");
      (opts.options || []).forEach(function (opt, idx) {
        if (buttons[idx]) {
          if (opt.key === selected) buttons[idx].classList.add("selected");
          else buttons[idx].classList.remove("selected");
        }
      });
    };
    return wrap;
  }

  function slider(opts) {
    opts = opts || {};
    var wrap = h("div", { className: "m3-slider" });

    var header = h("div", { className: "m3-slider-header" });
    var titleRow = h("div", { style: { display: "flex", alignItems: "center", gap: "6px" } });
    var titleText = h("span", { className: "label-medium text-muted", text: opts.label || "" });
    titleRow.appendChild(titleText);

    if (opts.onInfo) {
      var infoBtn = iconButton({
        icon: "HelpOutline",
        size: 15,
        onClick: opts.onInfo
      });
      infoBtn.style.width = "22px";
      infoBtn.style.height = "22px";
      titleRow.appendChild(infoBtn);
    }
    header.appendChild(titleRow);

    var valueEl = h("span", { className: "label-large text-primary" });
    header.appendChild(valueEl);
    wrap.appendChild(header);

    var trackWrap = h("div", { className: "m3-slider-track-wrap" });
    var input = h("input", {
      className: "m3-slider-input",
      attrs: {
        type: "range",
        min: opts.min !== undefined ? opts.min : 0,
        max: opts.max !== undefined ? opts.max : 100,
        step: opts.step !== undefined ? opts.step : 1,
        value: opts.value !== undefined ? opts.value : 0
      }
    });
    if (opts.enabled === false) input.disabled = true;

    function formatVal(v) {
      if (opts.valueFormatter) return opts.valueFormatter(v);
      return String(v);
    }

    valueEl.textContent = formatVal(Number(input.value));

    input.addEventListener("input", function (e) {
      var val = Number(e.target.value);
      valueEl.textContent = formatVal(val);
      if (opts.onValueChange) opts.onValueChange(val);
    });

    trackWrap.appendChild(input);
    wrap.appendChild(trackWrap);

    wrap.setValue = function (val) {
      input.value = val;
      valueEl.textContent = formatVal(Number(val));
    };
    return wrap;
  }

  function button(opts) {
    opts = opts || {};
    var variant = opts.variant || "filled";
    var btn = h("button", {
      className: "m3-button " + variant + (opts.fullWidth ? " full-width" : "") + (opts.className ? " " + opts.className : ""),
      attrs: { type: "button" }
    });
    if (opts.enabled === false) btn.disabled = true;

    if (opts.icon) {
      var iconEl = typeof opts.icon === "string" ? window.Icons.el(opts.icon, 18) : opts.icon;
      btn.appendChild(iconEl);
    }
    if (opts.text) {
      var textSpan = h("span", { text: opts.text });
      btn.appendChild(textSpan);
    }
    if (opts.onClick) {
      btn.addEventListener("click", opts.onClick);
    }
    return btn;
  }

  function iconButton(opts) {
    opts = opts || {};
    var btn = h("button", {
      className: "m3-icon-button" + (opts.className ? " " + opts.className : ""),
      attrs: { type: "button", title: opts.title || "" }
    });
    if (opts.enabled === false) btn.disabled = true;

    var s = opts.size || 24;
    var iconEl = typeof opts.icon === "string" ? window.Icons.el(opts.icon, s) : opts.icon;
    btn.appendChild(iconEl);

    if (opts.onClick) {
      btn.addEventListener("click", opts.onClick);
    }
    return btn;
  }

  function chip(opts) {
    opts = opts || {};
    var c = h("button", {
      className: "m3-chip" + (opts.selected ? " selected" : ""),
      attrs: { type: "button" }
    });
    if (opts.enabled === false) c.disabled = true;
    c.appendChild(h("span", { text: opts.label || "" }));
    if (opts.onClick) {
      c.addEventListener("click", opts.onClick);
    }
    return c;
  }

  function divider() {
    return h("div", { className: "m3-divider" });
  }

  function dialog(opts) {
    opts = opts || {};
    var root = document.getElementById("dialog-root") || document.body;

    // Remember focused element so we can restore it when this dialog closes
    var priorFocus = document.activeElement;

    var overlay = h("div", { className: "dialog-overlay" });
    var card = h("div", {
      className: "dialog-card",
      attrs: { role: "dialog", "aria-modal": "true", "aria-labelledby": "" }
    });

    var header = h("div", { className: "dialog-header" });
    var titleId = "dlg-title-" + (Math.random() * 1e9 | 0);
    var title = h("h3", { className: "title-medium", text: opts.title || "" });
    title.id = titleId;
    card.setAttribute("aria-labelledby", titleId);
    header.appendChild(title);

    var closed = false;

    var closeBtn = iconButton({
      icon: "Close",
      size: 20,
      title: "Закрыть",
      onClick: function () { api.close(); }
    });
    header.appendChild(closeBtn);
    card.appendChild(header);

    var content = h("div", { className: "dialog-content" });
    if (opts.content instanceof Node) {
      content.appendChild(opts.content);
    } else if (typeof opts.content === "string") {
      content.appendChild(h("p", { className: "body-medium", text: opts.content }));
    }
    card.appendChild(content);

    if (Array.isArray(opts.actions) && opts.actions.length > 0) {
      var actions = h("div", { className: "dialog-actions" });
      opts.actions.forEach(function (act) {
        var btn = button({
          text: act.text,
          variant: act.variant || "text",
          onClick: function () {
            if (act.onClick) act.onClick(api);
            else api.close();
          }
        });
        actions.appendChild(btn);
      });
      card.appendChild(actions);
    }

    overlay.appendChild(card);
    root.appendChild(overlay);

    requestAnimationFrame(function () {
      overlay.classList.add("active");
    });

    // Only the topmost overlay (last child of root) handles Escape
    function onKey(e) {
      if (e.key === "Escape") {
        // Check we are still the topmost dialog
        var overlays = root.querySelectorAll(".dialog-overlay");
        var last = overlays[overlays.length - 1];
        if (last === overlay) {
          e.stopPropagation();
          api.close();
        }
      }
    }
    window.addEventListener("keydown", onKey);

    overlay.addEventListener("click", function (e) {
      if (e.target === overlay) api.close();
    });

    var api = {
      el: overlay,
      close: function () {
        if (closed) return;
        closed = true;
        window.removeEventListener("keydown", onKey);
        overlay.classList.remove("active");
        setTimeout(function () {
          if (overlay.parentNode) overlay.parentNode.removeChild(overlay);
          if (opts.onClose) opts.onClose();
          // Only restore focus if this was actually the topmost dialog
          // (i.e. no other overlay was opened on top of us after we closed)
          var remaining = root.querySelectorAll(".dialog-overlay");
          if (priorFocus && priorFocus.isConnected && priorFocus.focus
              && (remaining.length === 0 || remaining[remaining.length - 1].contains(priorFocus))) {
            try { priorFocus.focus(); } catch (_) {}
          }
        }, 180);
      }
    };
    return api;
  }

  function confirm(opts) {
    opts = opts || {};
    return new Promise(function (resolve) {
      var dlg = dialog({
        title: opts.title || "Подтверждение",
        content: opts.message || "",
        actions: [
          {
            text: opts.cancelText || "Отмена",
            variant: "text",
            onClick: function (d) {
              d.close();
              resolve(false);
            }
          },
          {
            text: opts.confirmText || "ОК",
            variant: "filled",
            onClick: function (d) {
              d.close();
              resolve(true);
            }
          }
        ],
        onClose: function () {
          resolve(false);
        }
      });
    });
  }

  function infoDialog(title, bodyTextOrEl) {
    return dialog({
      title: title,
      content: bodyTextOrEl,
      actions: [
        {
          text: "Понятно",
          variant: "filled",
          onClick: function (d) { d.close(); }
        }
      ]
    });
  }

  function toast(text, durationMs) {
    var root = document.getElementById("toast-root");
    if (!root) return;
    var t = h("div", { className: "m3-toast", text: text });
    root.appendChild(t);
    requestAnimationFrame(function () {
      t.classList.add("show");
    });
    setTimeout(function () {
      t.classList.remove("show");
      setTimeout(function () {
        if (t.parentNode) t.parentNode.removeChild(t);
      }, 180);
    }, durationMs || 2500);
  }

  function loadingState(label) {
    var wrap = h("div", { className: "loading-state" });
    var spinner = h("div", { className: "m3-spinner" });
    var textEl = h("span", { className: "body-medium text-muted", text: label || "Загрузка…" });
    wrap.appendChild(spinner);
    wrap.appendChild(textEl);
    return wrap;
  }

  function emptyState(opts) {
    opts = opts || {};
    var wrap = h("div", { className: "empty-state" });
    var iconName = opts.icon || "InfoOutlined";
    var iconEl = window.Icons.el(iconName, 36, { fill: "var(--md-sys-color-on-surface-variant)" });
    wrap.appendChild(iconEl);

    var title = h("h4", { className: "title-medium", text: opts.title || "" });
    wrap.appendChild(title);

    if (opts.description) {
      var desc = h("p", { className: "body-medium text-muted", text: opts.description });
      wrap.appendChild(desc);
    }
    return wrap;
  }

  function banner(opts) {
    opts = opts || {};
    var b = h("div", {
      className: "banner-box",
      style: {
        backgroundColor: opts.containerColor || "var(--md-sys-color-surface-variant)",
        color: opts.contentColor || "var(--md-sys-color-on-surface-variant)"
      }
    });
    if (opts.icon) {
      var icon = window.Icons.el(opts.icon, 20);
      b.appendChild(icon);
    }
    var txt = h("span", { className: "body-medium", style: { flex: "1" }, text: opts.text || "" });
    b.appendChild(txt);
    if (opts.action) {
      b.appendChild(opts.action);
    }
    return b;
  }

  window.UI = {
    h: h,
    sectionCard: sectionCard,
    settingRow: settingRow,
    switchRow: settingRow,
    switchWidget: switchWidget,
    textField: textField,
    dropdown: dropdown,
    segmented: segmented,
    slider: slider,
    button: button,
    iconButton: iconButton,
    chip: chip,
    divider: divider,
    dialog: dialog,
    confirm: confirm,
    infoDialog: infoDialog,
    toast: toast,
    loadingState: loadingState,
    emptyState: emptyState,
    banner: banner
  };
})();
