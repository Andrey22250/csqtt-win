// Bridge between the page and the host (csqtt.exe, src/gui.rs). Classic script, no modules:
// the page is opened by file URL. Everything the UI needs from the outside world goes
// through `window.csqtt`.
//
// API
//   csqtt.ipc(request)            -> Promise<response>   one request to the service. `request`
//                                    is a csqtt-ipc Request object, e.g. {cmd: "status"},
//                                    {cmd: "connect"}, {cmd: "get_settings"},
//                                    {cmd: "set_settings", settings}, {cmd: "set_secret", key, value},
//                                    {cmd: "import", text}, {cmd: "export", include_secrets},
//                                    {cmd: "logs", follow: false, tail: 500}.
//                                    Resolves with the Response object ({type: "ok"},
//                                    {type: "status", status}, {type: "settings", settings},
//                                    {type: "exported", json}, {type: "logs", lines}).
//                                    Rejects with Error(message) on {type: "error"} or when the
//                                    service is not running.
//   csqtt.connect(), csqtt.disconnect(), csqtt.status()        shortcuts, return the status object
//                                    where there is one
//   csqtt.getSettings()           -> Promise<settings object>
//   csqtt.setSettings(settings)   -> Promise<void>  (send the full object from getSettings)
//   csqtt.updateSettings(fn)      -> Promise<settings>  get, let fn(settings) mutate it, set
//   csqtt.setSecret(key, value)   -> Promise<void>
//   csqtt.importText(text)        -> Promise<void>
//   csqtt.exportJson(includeSecrets) -> Promise<string>
//   csqtt.logsTail(n)             -> Promise<string[]>
//   csqtt.version()               -> Promise<string>
//   csqtt.parseLink(text)        -> Promise<{host, port, hashes, has_token} | null>  decodes a
//                                    csqtt:// link with the service's parser (base64 of an AES-GCM
//                                    payload); null when invalid. Password/token are never returned.
//   csqtt.linkInfo(text)          -> sync, cached: {host, peer, hashes: Array(n), vkAccessToken}
//                                    for a valid link, null for an invalid one, undefined while the
//                                    first parseLink of this text is in flight (a "settings" event
//                                    is emitted when it lands, so screens re-sync).
//   csqtt.setupService()          -> Promise<void>  installs (if needed) and starts the service:
//                                    runs `csqtt-svc.exe setup` elevated (UAC prompt). Rejects with
//                                    Error(message) when the user declines UAC or setup fails.
//   csqtt.openUrl(url)            -> Promise<void>  http/https only, opens the default browser
//   csqtt.runSelf(args, {env, onOutput}) -> {proc: Promise<number>, done: Promise<{code}>,
//                                    input(text), kill()}   starts `csqtt <args>` hidden.
//                                    Allowed first argument: vk-login, vk-logout, deploy,
//                                    undeploy, update-check. `env` may only hold CSQTT_* keys.
//                                    onOutput(text) gets raw output chunks (stdout and stderr,
//                                    not split into lines; a prompt may come without a newline).
//   csqtt.on(event, fn) / csqtt.off(event, fn)
//
// Events
//   "service"  {running: bool}        the service appeared / disappeared
//   "status"   {status: StatusInfo}   {state: "stopped"|"starting"|"connected"|"reconnecting"|
//                                      "paused"|"failed", message, client_ip, dns,
//                                      active_workers, bytes_up, bytes_down}
//   "log"      {line: string}         one new log line
//   "captcha"  {active: bool}         a captcha window was opened / finished
//
// Without the host (page opened in a normal browser) the bridge switches to the mock from
// mock.js, so every screen can be developed and checked in a browser.

(function () {
  "use strict";

  var listeners = {};
  var pending = {};
  var nextId = 1;
  var linkCache = {};
  var linkPending = {};
  var hosted = !!(window.chrome && window.chrome.webview);

  function emit(event, data) {
    var list = listeners[event];
    if (!list) return;
    list.slice().forEach(function (fn) {
      try {
        fn(data);
      } catch (err) {
        console.error("csqtt listener for " + event, err);
      }
    });
  }

  function call(method, params) {
    if (!hosted) return window.csqttMock.call(method, params || {}, emit);
    return new Promise(function (resolve, reject) {
      var id = nextId++;
      pending[id] = { resolve: resolve, reject: reject };
      window.chrome.webview.postMessage({ id: id, method: method, params: params || {} });
    });
  }

  // Calls made through `callWithId` need the id before the promise settles (process control).
  function callWithId(method, params) {
    if (!hosted) {
      var mockId = nextId++;
      return { id: mockId, promise: window.csqttMock.call(method, params || {}, emit, mockId) };
    }
    var id = nextId++;
    var promise = new Promise(function (resolve, reject) {
      pending[id] = { resolve: resolve, reject: reject };
    });
    window.chrome.webview.postMessage({ id: id, method: method, params: params || {} });
    return { id: id, promise: promise };
  }

  window.__csqttHost = {
    recv: function (messages) {
      for (var i = 0; i < messages.length; i++) {
        var message = messages[i];
        if (message.event) {
          emit(message.event, message.data);
          continue;
        }
        var entry = pending[message.id];
        if (!entry) continue;
        delete pending[message.id];
        if (message.ok) entry.resolve(message.result);
        else entry.reject(new Error(message.error || "ошибка"));
      }
    },
  };

  function ipc(request) {
    return call("ipc", { request: request });
  }

  var csqtt = {
    hosted: hosted,
    call: call,
    ipc: ipc,
    on: function (event, fn) {
      (listeners[event] = listeners[event] || []).push(fn);
    },
    off: function (event, fn) {
      var list = listeners[event];
      if (!list) return;
      var index = list.indexOf(fn);
      if (index >= 0) list.splice(index, 1);
    },
    connect: function () {
      return ipc({ cmd: "connect" });
    },
    disconnect: function () {
      return ipc({ cmd: "disconnect" });
    },
    status: function () {
      return ipc({ cmd: "status" }).then(function (r) {
        return r.status;
      });
    },
    getSettings: function () {
      return ipc({ cmd: "get_settings" }).then(function (r) {
        return r.settings;
      });
    },
    setSettings: function (settings) {
      return ipc({ cmd: "set_settings", settings: settings }).then(function () {});
    },
    updateSettings: function (mutate) {
      return csqtt.getSettings().then(function (settings) {
        mutate(settings);
        return csqtt.setSettings(settings).then(function () {
          emit("settings", { settings: settings });
          return settings;
        });
      });
    },
    setSecret: function (key, value) {
      return ipc({ cmd: "set_secret", key: key, value: value }).then(function () {});
    },
    importText: function (text) {
      return ipc({ cmd: "import", text: text }).then(function () {
        return csqtt.getSettings().then(function (settings) {
          emit("settings", { settings: settings });
        });
      });
    },
    exportJson: function (includeSecrets) {
      return ipc({ cmd: "export", include_secrets: !!includeSecrets }).then(function (r) {
        return r.json;
      });
    },
    logsTail: function (count) {
      return ipc({ cmd: "logs", follow: false, tail: count || 500 }).then(function (r) {
        return r.lines || [];
      });
    },
    version: function () {
      return call("version").then(function (r) {
        return r.version;
      });
    },
    linkInfo: function (text) {
      var key = String(text || "").replace(/\s+/g, "");
      if (!key) return null;
      if (Object.prototype.hasOwnProperty.call(linkCache, key)) return linkCache[key];
      if (!linkPending[key]) {
        linkPending[key] = true;
        csqtt.parseLink(key).then(
          function (info) {
            linkCache[key] = info
              ? {
                  host: info.host,
                  peer: String(info.port),
                  hashes: new Array(info.hashes || 0).fill(""),
                  vkAccessToken: info.has_token ? "*" : "",
                }
              : null;
            delete linkPending[key];
            var app = window.App;
            if (app && app.state && app.state.settings) emit("settings", { settings: app.state.settings });
          },
          function () {
            delete linkPending[key];
          }
        );
      }
      return undefined;
    },
    parseLink: function (text) {
      return call("parse_link", { link: text || "" });
    },
    setupService: function () {
      return call("service_setup").then(function () {});
    },
    openUrl: function (url) {
      return call("open_url", { url: url });
    },
    runSelf: function (args, options) {
      options = options || {};
      var started = callWithId("proc_start", { args: args, env: options.env || {} });
      var onOutput = function (data) {
        if (data.proc === started.id && options.onOutput) options.onOutput(data.text);
      };
      csqtt.on("proc_output", onOutput);
      var done = started.promise.then(
        function (result) {
          csqtt.off("proc_output", onOutput);
          return result;
        },
        function (err) {
          csqtt.off("proc_output", onOutput);
          throw err;
        }
      );
      return {
        proc: started.id,
        done: done,
        input: function (text) {
          return call("proc_input", { proc: started.id, text: text });
        },
        kill: function () {
          return call("proc_kill", { proc: started.id });
        },
      };
    },
  };

  window.csqtt = csqtt;

  window.addEventListener("DOMContentLoaded", function () {
    if (hosted) window.chrome.webview.postMessage({ method: "ready" });
    else if (window.csqttMock) window.csqttMock.start(emit);
  });
})();
