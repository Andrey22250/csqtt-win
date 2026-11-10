// Icons repository for csqtt-win.
// Provides inline SVG path data and Icons.el(name, size, options) returning an <svg> DOM element.

(function () {
  "use strict";

  var SVG_NS = "http://www.w3.org/2000/svg";

  var paths = {
    PowerSettingsNew: "M13 3h-2v10h2V3zm4.83 2.17l-1.42 1.42A6.92 6.92 0 0 1 19 12c0 3.87-3.13 7-7 7s-7-3.13-7-7c0-2.38 1.19-4.47 3-5.74L6.58 4.84C4.38 6.57 3 9.13 3 12c0 4.97 4.03 9 9 9s9-4.03 9-9c0-2.87-1.38-5.43-3.17-7.17z",
    VpnKey: "M12.65 10C11.83 7.67 9.61 6 7 6c-3.31 0-6 2.69-6 6s2.69 6 6 6c2.61 0 4.83-1.67 5.65-4H17v4h4v-4h2v-4H12.65zM7 14c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z",
    VpnKeyOutlined: "M7 14c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm0-6C4.79 8 3 9.79 3 12s1.79 4 4 4c1.9 0 3.51-1.33 3.91-3.1H18v2h2v-2h1v-2H10.91C10.51 9.33 8.9 8 7 8zm14 2h-1.59l-.41.41V12h-2v-1.59l-.41-.41H11.5l-.23.77C10.74 13.06 9 14.5 7 14.5c-1.38 0-2.5-1.12-2.5-2.5s1.12-2.5 2.5-2.5c2 0 3.74 1.44 4.27 3.32l.23.77h8.5V10z",
    Cloud: "M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96z",
    CloudOutlined: "M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM19 18H6c-2.21 0-4-1.79-4-4 0-2.05 1.53-3.76 3.56-3.97l1.07-.11.5-.95C8.08 7.14 9.94 6 12 6c2.62 0 4.88 1.86 5.39 4.43l.3 1.5 1.53.11c1.56.1 2.78 1.41 2.78 2.96 0 1.65-1.35 3-3 3z",
    Terminal: "M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm-12.5 11l-1.4-1.4 3.1-3.1-3.1-3.1 1.4-1.4 4.5 4.5-4.5 4.5zm8.5 0h-6v-2h6v2z",
    TerminalOutlined: "M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 14H4V6h16v12zm-12.5-3l-1.4-1.4 3.1-3.1-3.1-3.1 1.4-1.4 4.5 4.5-4.5 4.5zm8.5 0h-6v-2h6v2z",
    FilterList: "M10 18h4v-2h-4v2zM3 6v2h18V6H3zm3 7h12v-2H6v2z",
    FilterListOutlined: "M10 18h4v-2h-4v2zM3 6v2h18V6H3zm3 7h12v-2H6v2z",
    Info: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z",
    InfoOutlined: "M11 7h2v2h-2zm0 4h2v6h-2zm1-9C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8z",
    Close: "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z",
    Delete: "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z",
    DeleteOutlined: "M16 9v10H8V9h8m-1.5-6h-5l-1 1H5v2h14V4h-3.5l-1-1zM18 7H6v12c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7z",
    ContentCopy: "M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z",
    Check: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z",
    CheckCircle: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z",
    Phone: "M6.62 10.79c1.44 2.83 3.76 5.14 6.59 6.59l2.2-2.2c.27-.27.67-.36 1.02-.24 1.12.37 2.33.57 3.57.57.55 0 1 .45 1 1V20c0 .55-.45 1-1 1-9.39 0-17-7.61-17-17 0-.55.45-1 1-1h3.5c.55 0 1 .45 1 1 0 1.25.2 2.45.57 3.57.11.35.03.74-.25 1.02l-2.2 2.2z",
    Key: "M21 10h-8.35C11.83 7.67 9.61 6 7 6c-3.31 0-6 2.69-6 6s2.69 6 6 6c2.61 0 4.83-1.67 5.65-4H17v4h4v-4h2v-4h-2zm-14 4c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z",
    Link: "M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z",
    Login: "M11 7L9.6 8.4l2.6 2.6H2v2h10.2l-2.6 2.6L11 17l5-5-5-5zm9 12h-8v2h8c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2h-8v2h8v14z",
    HelpOutline: "M11 18h2v-2h-2v2zm1-16C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm0-14c-2.21 0-4 1.79-4 4h2c0-1.1.9-2 2-2s2 .9 2 2c0 2-3 1.75-3 5h2c0-2.25 3-2.5 3-5 0-2.21-1.79-4-4-4z",
    Dns: "M20 13H4c-.55 0-1 .45-1 1v6c0 .55.45 1 1 1h16c.55 0 1-.45 1-1v-6c0-.55-.45-1-1-1zM7 19c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zM20 3H4c-.55 0-1 .45-1 1v6c0 .55.45 1 1 1h16c.55 0 1-.45 1-1V4c0-.55-.45-1-1-1zM7 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z",
    Tag: "M20 10V8h-4V4h-2v4h-4V4H8v4H4v2h4v4H4v2h4v4h2v-4h4v4h2v-4h4v-2h-4v-4h4zm-6 4h-4v-4h4v4z",
    ArrowDownward: "M20 12l-1.41-1.41L13 16.17V4h-2v12.17l-5.58-5.59L4 12l8 8 8-8z",
    Visibility: "M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z",
    VisibilityOff: "M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.43-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7zM2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3 2 4.27zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2zm4.31-.78l3.15 3.15.02-.16c0-1.66-1.34-3-3-3l-.17.01z",
    Settings: "M19.14,12.94c0.04,-0.3 0.06,-0.61 0.06,-0.94c0,-0.32 -0.02,-0.64 -0.07,-0.94l2.03,-1.58c0.18,-0.14 0.23,-0.41 0.12,-0.61l-1.92,-3.32c-0.12,-0.22 -0.37,-0.29 -0.59,-0.22l-2.39,0.96c-0.5,-0.38 -1.03,-0.7 -1.62,-0.94L14.4,2.81c-0.04,-0.24 -0.24,-0.41 -0.48,-0.41h-3.84c-0.24,0 -0.43,0.17 -0.47,0.41L9.25,5.35C8.66,5.59 8.12,5.92 7.63,6.29L5.24,5.33c-0.22,-0.08 -0.47,0 -0.59,0.22L2.74,8.87c-0.12,0.21 -0.08,0.47 0.12,0.61l2.03,1.58c-0.05,0.3 -0.09,0.63 -0.09,0.94s0.02,0.64 0.07,0.94l-2.03,1.58c-0.18,0.14 -0.23,0.41 -0.12,0.61l1.92,3.32c0.12,0.22 0.37,0.29 0.59,0.22l2.39,-0.96c0.5,0.38 1.03,0.7 1.62,0.94l0.36,2.54c0.05,0.24 0.24,0.41 0.48,0.41h3.84c0.24,0 0.44,-0.17 0.47,-0.41l0.36,-2.54c0.59,-0.24 1.13,-0.56 1.62,-0.94l2.39,0.96c0.22,0.08 0.47,0 0.59,-0.22l1.92,-3.32c0.12,-0.22 0.07,-0.47 -0.12,-0.61L19.14,12.94z M12,15.6c-1.98,0 -3.6,-1.62 -3.6,-3.6s1.62,-3.6 3.6,-3.6s3.6,1.62 3.6,3.6S13.98,15.6 12,15.6z",
    ImportExport: "M9,3L5,6.99h3V14h2V6.99h3L9,3z M16,17.01V10h-2v7.01h-3L15,21l4,-3.99h-3z",
    QrCode: "M3 3h8v8H3zm2 2v4h4V5zm8-2h8v8h-8zm2 2v4h4V5zM3 13h8v8H3zm2 2v4h4v-4zm13-2h3v2h-3zm-3 2h2v3h-2zm2 3h2v2h-2zm-2 2h3v2h-3zm4-2h2v4h-2zm-2-5h4v2h-4z",
    Heart: "M1907.077,623.957c-7.101,-50.137 -23.803,-99.099 -48.806,-143.131c-69.212,-122.899 -202.469,-205.261 -342.732,-215.692c-171.634,-13.137 -353.376,80.583 -421.922,240.928c-0.811,1.953 -1.594,3.922 -2.363,5.897c-16.566,-49.396 -39.517,-96.63 -71.107,-139.012c-68.567,-93.198 -180.972,-156.408 -298.389,-155.517c-71.343,-0.13 -141.959,20.619 -203.416,56.365c-94.805,54.402 -160.846,142.236 -195.136,245.098c-52.665,165.96 -42.61,345.635,14.526,508.365c56.27,158.434,161.254,297.184,289.169,404.657c71.421,60.754,152.256,114.557,245.384,133.342c17.029,3.009,28.09,-17.229,17.02,-29.863c-5.776,-6.665 -12.016,-13.091 -18.165,-19.229c-56.013,-56.551 -119.935,-104.78 -177.527,-158.861C506.717,1181.782,400.501,938.069,431.95,680.964c10.058,-86.793,35.089,-171.062,94.389,-235.475c52.092,-55.504,127.221,-91.948,203.356,-92.293c79.492,0.553,168.944,34.491,228.991,85.745c36.003,30.673,91.267,101.9,115.442,152.345c7.838,16.356,25.872,12.91,34.839,0.012c7.528,-10.812,19.471,-28.839,26.643,-37.866c90.292,-112.164,242.564,-172.678,383.661,-149.864c76.393,13.61,147.442,57.757,191.767,121.65c34.469,49.06,50.334,108.641,46.841,168.362c-12.014,278.608 -281.102,522.53 -466.215,713.44c-149.127,154.103 -349.689,332.482 -378.879,556.053c-1.11,20.284,23.159,25.675,32.427,11.774c61.106,-88.489,133.257,-165.095,208.583,-241.54c113.7,-117.972,237.446,-226.411,355.996,-340.573c144.998,-141.378,288.238,-294.534,360.88,-488.172C1903.085,815.791,1919.477,718.441,1907.077,623.957z",
    ChevronDown: "M7 10l5 5 5-5z"
  };

  var viewBoxes = {
    Heart: "0 0 2200 2200",
    CsqttLogo: "0 0 760 184",
    CsqttSublogo: "0 0 516 64",
    ConnectLogo: "0 0 108 108"
  };

  function el(name, size, options) {
    options = options || {};
    var s = size || 24;
    var svg = document.createElementNS(SVG_NS, "svg");
    var viewBox = viewBoxes[name] || "0 0 24 24";
    var vb = viewBox.split(" ");
    // `size` is the height; wide logos keep their aspect ratio instead of shrinking into a square.
    svg.setAttribute("width", String(Math.round((s * Number(vb[2])) / Number(vb[3]))));
    svg.setAttribute("height", String(s));
    svg.setAttribute("viewBox", viewBox);
    svg.setAttribute("fill", options.fill || "currentColor");
    svg.setAttribute("aria-hidden", "true");
    if (options.className) svg.setAttribute("class", options.className);

    if (name === "ConnectLogo") {
      buildConnectLogo(svg);
      return svg;
    }

    if (name === "CsqttLogo") {
      buildCsqttLogo(svg);
      return svg;
    }

    if (name === "CsqttSublogo") {
      buildCsqttSublogo(svg);
      return svg;
    }

    var pathData = paths[name];
    if (pathData) {
      var path = document.createElementNS(SVG_NS, "path");
      path.setAttribute("d", pathData);
      svg.appendChild(path);
    }
    return svg;
  }

  function buildConnectLogo(svg) {
    var defs = document.createElementNS(SVG_NS, "defs");

    var radialGrad = document.createElementNS(SVG_NS, "radialGradient");
    radialGrad.setAttribute("id", "csqtt-logo-radial");
    radialGrad.setAttribute("cx", "50%");
    radialGrad.setAttribute("cy", "50%");
    radialGrad.setAttribute("r", "50%");

    var stopR1 = document.createElementNS(SVG_NS, "stop");
    stopR1.setAttribute("offset", "0%");
    stopR1.setAttribute("stop-color", "#0474fb");
    stopR1.setAttribute("stop-opacity", "0.22");

    var stopR2 = document.createElementNS(SVG_NS, "stop");
    stopR2.setAttribute("offset", "60%");
    stopR2.setAttribute("stop-color", "#025ecb");
    stopR2.setAttribute("stop-opacity", "0.08");

    var stopR3 = document.createElementNS(SVG_NS, "stop");
    stopR3.setAttribute("offset", "100%");
    stopR3.setAttribute("stop-color", "#000000");
    stopR3.setAttribute("stop-opacity", "0");

    radialGrad.appendChild(stopR1);
    radialGrad.appendChild(stopR2);
    radialGrad.appendChild(stopR3);
    defs.appendChild(radialGrad);

    var linearGrad = document.createElementNS(SVG_NS, "linearGradient");
    linearGrad.setAttribute("id", "csqtt-logo-linear");
    linearGrad.setAttribute("x1", "28%");
    linearGrad.setAttribute("y1", "36%");
    linearGrad.setAttribute("x2", "78%");
    linearGrad.setAttribute("y2", "72%");

    var stopL1 = document.createElementNS(SVG_NS, "stop");
    stopL1.setAttribute("offset", "0%");
    stopL1.setAttribute("stop-color", "#4FA3FF");

    var stopL2 = document.createElementNS(SVG_NS, "stop");
    stopL2.setAttribute("offset", "100%");
    stopL2.setAttribute("stop-color", "#0474fb");

    linearGrad.appendChild(stopL1);
    linearGrad.appendChild(stopL2);
    defs.appendChild(linearGrad);
    svg.appendChild(defs);

    var glowCircle = document.createElementNS(SVG_NS, "circle");
    glowCircle.setAttribute("cx", "54");
    glowCircle.setAttribute("cy", "54");
    glowCircle.setAttribute("r", "40");
    glowCircle.setAttribute("fill", "url(#csqtt-logo-radial)");
    glowCircle.setAttribute("class", "csqtt-logo-glow");
    svg.appendChild(glowCircle);

    var cPath = document.createElementNS(SVG_NS, "path");
    cPath.setAttribute("d", "M 78.6,36.8 A 30,30 0 1,0 78.6,71.2 L 69.6,64.9 A 19,19 0 1,1 69.6,43.1 Z");
    cPath.setAttribute("fill", "url(#csqtt-logo-linear)");
    cPath.setAttribute("class", "csqtt-logo-body");
    svg.appendChild(cPath);
  }

  function buildCsqttLogo(svg) {
    var defs = document.createElementNS(SVG_NS, "defs");
    var linearGrad = document.createElementNS(SVG_NS, "linearGradient");
    linearGrad.setAttribute("id", "csqtt-brand-grad");
    linearGrad.setAttribute("x1", "0%");
    linearGrad.setAttribute("y1", "0%");
    linearGrad.setAttribute("x2", "100%");
    linearGrad.setAttribute("y2", "100%");

    var stops = [
      { offset: "0%", color: "#3DBFFD" },
      { offset: "32%", color: "#299BFD" },
      { offset: "66%", color: "#1475FD" },
      { offset: "100%", color: "#0450FC" }
    ];
    stops.forEach(function (s) {
      var stop = document.createElementNS(SVG_NS, "stop");
      stop.setAttribute("offset", s.offset);
      stop.setAttribute("stop-color", s.color);
      linearGrad.appendChild(stop);
    });
    defs.appendChild(linearGrad);
    svg.appendChild(defs);

    var p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("fill-rule", "evenodd");
    p.setAttribute("fill", "url(#csqtt-brand-grad)");
    p.setAttribute("d", "M79,11 L66,15 L48,25 L40,32 L27,49 L21,62 L17,78 L17,98 L19,108 L22,117 L33,136 L40,144 L56,156 L66,161 L84,166 L108,166 L118,164 L127,161 L143,152 L157,139 L160,135 L160,133 L137,116 L135,116 L123,128 L113,133 L106,135 L100,135 L99,136 L87,135 L73,130 L67,126 L57,116 L51,105 L48,93 L48,83 L51,71 L58,59 L67,50 L83,42 L105,41 L115,44 L129,53 L136,61 L160,44 L160,42 L155,35 L144,25 L135,19 L124,14 L108,10 Z M219,13 L208,18 L201,23 L191,35 L186,49 L186,66 L188,73 L193,82 L200,89 L208,94 L219,98 L248,103 L264,108 L270,113 L272,117 L272,125 L267,132 L260,136 L255,137 L234,137 L220,133 L212,129 L199,119 L181,141 L199,155 L212,161 L222,164 L232,165 L233,166 L257,166 L276,161 L285,156 L292,150 L298,142 L302,132 L303,112 L297,97 L287,87 L276,81 L258,76 L234,72 L224,68 L218,63 L216,58 L217,50 L222,44 L228,41 L237,39 L248,39 L258,41 L268,45 L279,53 L281,53 L299,32 L285,21 L270,14 L259,11 L233,10 Z M354,25 L344,34 L333,49 L328,60 L324,74 L324,79 L323,80 L324,103 L329,119 L336,132 L349,147 L360,155 L372,161 L391,166 L414,166 L430,162 L442,156 L460,174 L478,156 L462,140 L469,131 L474,122 L479,109 L481,99 L481,77 L479,68 L474,54 L466,41 L453,27 L441,19 L430,14 L414,10 L385,11 L372,15 Z M389,42 L411,41 L427,47 L442,61 L449,76 L450,96 L447,106 L444,112 L439,118 L421,100 L404,116 L405,119 L418,132 L416,134 L399,136 L385,133 L377,129 L363,116 L355,99 L354,83 L357,71 L361,63 L373,50 L379,46 Z M496,13 L496,44 L536,44 L537,45 L537,163 L568,163 L568,45 L569,44 L609,44 L610,43 L610,13 Z M628,13 L627,14 L627,37 L628,38 L627,43 L628,44 L666,44 L667,45 L667,163 L698,163 L699,162 L699,45 L700,44 L738,44 L738,13 Z");
    svg.appendChild(p);
  }

  function buildCsqttSublogo(svg) {
    var defs = document.createElementNS(SVG_NS, "defs");
    var linearGrad = document.createElementNS(SVG_NS, "linearGradient");
    linearGrad.setAttribute("id", "csqtt-sublogo-grad");
    linearGrad.setAttribute("x1", "0%");
    linearGrad.setAttribute("y1", "0%");
    linearGrad.setAttribute("x2", "100%");
    linearGrad.setAttribute("y2", "100%");

    var stops = [
      { offset: "0%", color: "#45B7FF" },
      { offset: "45%", color: "#2087FF" },
      { offset: "100%", color: "#075BFF" }
    ];
    stops.forEach(function (s) {
      var stop = document.createElementNS(SVG_NS, "stop");
      stop.setAttribute("offset", s.offset);
      stop.setAttribute("stop-color", s.color);
      linearGrad.appendChild(stop);
    });
    defs.appendChild(linearGrad);
    svg.appendChild(defs);

    var p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("fill", "none");
    p.setAttribute("stroke", "url(#csqtt-sublogo-grad)");
    p.setAttribute("stroke-width", "6");
    p.setAttribute("stroke-linecap", "round");
    p.setAttribute("stroke-linejoin", "round");
    p.setAttribute("d", "M8,24 L18,48 L28,24 M34,20 Q38,14 46,14 H50 Q58,14 58,22 Q58,28 52,33 L36,48 H58 M67,47 L67.8,47 M102,14 H80 V28 H92 Q102,28 102,38 Q102,48 90,48 Q80,48 78,42 M110,47 L110.8,47 M122,22 Q122,14 134,14 Q146,14 146,22 V40 Q146,48 134,48 Q122,48 122,40 Z M158,12 V48 M158,31 Q163,24 170,24 Q180,24 180,36 Q180,48 170,48 Q163,48 158,41 M190,24 L200,47 M210,24 L200,47 L196,57 Q194,60 189,58 M250,30 Q246,24 239,24 Q228,24 228,36 Q228,48 239,48 Q246,48 250,42 M250,24 V48 M260,48 V24 M260,31 Q264,24 270,24 Q277,24 277,32 V48 M277,31 Q281,24 287,24 Q294,24 294,32 V48 M304,24 V39 Q304,48 314,48 Q324,48 324,39 V24 M334,48 V24 M334,32 Q338,24 348,24 M382,29 Q378,24 370,24 Q360,24 360,36 Q360,48 370,48 Q378,48 382,43 M414,30 Q410,24 403,24 Q392,24 392,36 Q392,48 403,48 Q410,48 414,42 M414,24 V48 M424,48 V24 M424,32 Q429,24 436,24 Q446,24 446,35 V48 M467,24 Q456,24 456,36 Q456,48 467,48 Q478,48 478,36 Q478,24 467,24 Z M488,24 L498,48 L508,24");
    svg.appendChild(p);
  }

  window.Icons = {
    paths: paths,
    viewBoxes: viewBoxes,
    el: el
  };
})();
