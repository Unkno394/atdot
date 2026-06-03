(function (window, document) {
  'use strict';

  var ENDPOINT = 'https://api.ru';
  var FLUSH_INTERVAL = 2000;   // ms
  var MOUSE_SAMPLE   = 100;    // ms

  // ── helpers ───────────────────────────────────────────────────────────────

  function uuid() {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function (c) {
      var r = (Math.random() * 16) | 0;
      return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16);
    });
  }

  // visitor_id: cookie takes priority over localStorage (survives incognito wipe + ITP)
  function getOrCreateVisitorId() {
    var m = document.cookie.match(/mdr_vid=([^;]+)/);
    if (m) return m[1];
    var v = null;
    try { v = localStorage.getItem('mdr_vid'); } catch (e) {}
    if (!v) v = uuid();
    try { localStorage.setItem('mdr_vid', v); } catch (e) {}
    document.cookie = 'mdr_vid=' + v + ';max-age=31536000;path=/;SameSite=Lax';
    return v;
  }

  function canvasFingerprint() {
    try {
      var c   = document.createElement('canvas');
      var ctx = c.getContext('2d');
      ctx.textBaseline = 'top';
      ctx.font         = '14px Arial';
      ctx.fillStyle    = '#f60';
      ctx.fillRect(0, 0, 10, 10);
      ctx.fillStyle    = '#069';
      ctx.fillText('atdot🔒', 2, 2);
      ctx.strokeStyle = 'rgba(102,204,0,0.7)';
      ctx.beginPath(); ctx.arc(5, 5, 4, 0, Math.PI * 2); ctx.stroke();
      return c.toDataURL().slice(-50);
    } catch (e) { return ''; }
  }

  function buildFingerprint() {
    var nav   = window.navigator;
    var parts = [
      nav.userAgent,
      nav.language,
      screen.width + 'x' + screen.height + 'x' + screen.colorDepth,
      new Date().getTimezoneOffset(),
      nav.hardwareConcurrency || '',
      nav.deviceMemory        || '',
      (nav.plugins || []).length,
      canvasFingerprint(),
    ];
    var s = parts.join('|'), h = 0;
    for (var i = 0; i < s.length; i++) {
      h = Math.imul(31, h) + s.charCodeAt(i) | 0;
    }
    return (h >>> 0).toString(16);
  }

  function linearity(pts) {
    if (pts.length < 2) return 0;
    var dx = pts[pts.length - 1].x - pts[0].x;
    var dy = pts[pts.length - 1].y - pts[0].y;
    var direct = Math.sqrt(dx * dx + dy * dy);
    var total  = 0;
    for (var i = 1; i < pts.length; i++) {
      var ddx = pts[i].x - pts[i - 1].x;
      var ddy = pts[i].y - pts[i - 1].y;
      total += Math.sqrt(ddx * ddx + ddy * ddy);
    }
    return total > 0 ? Math.min(direct / total, 1) : 0;
  }

  function directionChanges(pts) {
    if (pts.length < 3) return 0;
    var changes = 0, pdx = pts[1].x - pts[0].x, pdy = pts[1].y - pts[0].y;
    for (var i = 2; i < pts.length; i++) {
      var dx = pts[i].x - pts[i - 1].x, dy = pts[i].y - pts[i - 1].y;
      if (pdx * dx + pdy * dy < 0) changes++;
      pdx = dx; pdy = dy;
    }
    return changes;
  }

  function velocities(pts, intervalMs) {
    var result = [];
    for (var i = 1; i < pts.length; i++) {
      var dx = pts[i].x - pts[i - 1].x, dy = pts[i].y - pts[i - 1].y;
      result.push(Math.sqrt(dx * dx + dy * dy) / intervalMs);
    }
    return result;
  }

  function fittsID(startX, startY, rectCx, rectCy, targetW, targetH) {
    var d = Math.sqrt(Math.pow(rectCx - startX, 2) + Math.pow(rectCy - startY, 2));
    var w = Math.max(Math.min(targetW, targetH), 1);
    return Math.log2(1 + d / w);
  }

  // ── WebRTC IP leak detection ──────────────────────────────────────────────

  function detectWebRtcIps(cb) {
    var result = { webrtc_ip: null, ipv6: null };
    if (!window.RTCPeerConnection) { cb(result); return; }
    var pc   = new RTCPeerConnection({ iceServers: [{ urls: 'stun:stun.l.google.com:19302' }] });
    var seen = {};
    pc.createDataChannel('');
    pc.onicecandidate = function (e) {
      if (!e || !e.candidate) { pc.close(); cb(result); return; }
      var m = e.candidate.candidate.match(
        /(\d+\.\d+\.\d+\.\d+|[0-9a-f]{0,4}(?::[0-9a-f]{0,4}){2,7})/gi
      );
      if (!m) return;
      m.forEach(function (ip) {
        if (seen[ip]) return;
        seen[ip] = true;
        if (ip.indexOf(':') !== -1) {
          if (!result.ipv6) result.ipv6 = ip;
        } else if (!/^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|127\.)/.test(ip)) {
          if (!result.webrtc_ip) result.webrtc_ip = ip;
        }
      });
    };
    pc.createOffer().then(function (o) { return pc.setLocalDescription(o); });
    setTimeout(function () { pc.close(); cb(result); }, 2000);
  }

  // ── core ──────────────────────────────────────────────────────────────────

  var atdot = {
    _key:          null,
    _sessionId:    null,
    _visitorId:    null,
    _userId:       null,    // set via atdot.identify(id)
    _queue:        [],
    _mouseBuffer:  [],
    _lastTs:       Date.now(),
    _flushTimer:   null,
    _mouseTimer:   null,
    _webrtc:       null,
    _timezone:     null,
    _fingerprint:  null,
    _hoverStart:   {},
    _seenElements: {},
    _mouseStart:   null,

    init: function (apiKey, options) {
      if (!apiKey) { console.warn('[atdot] no API key'); return; }
      this._key         = apiKey;
      this._sessionId   = uuid();
      this._visitorId   = getOrCreateVisitorId();
      this._timezone    = Intl.DateTimeFormat().resolvedOptions().timeZone || null;
      this._fingerprint = buildFingerprint();

      if (options && options.endpoint) ENDPOINT = options.endpoint;

      var self = this;
      detectWebRtcIps(function (r) { self._webrtc = r; });

      this._bind();
      this._flushTimer = setInterval(function () { atdot._flush(); }, FLUSH_INTERVAL);
      this.track('page_view', { url: location.href, referrer: document.referrer });

      // DOM perturbation — defer so framework-rendered forms are present
      if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', function () { self._perturb(); });
      } else {
        setTimeout(function () { self._perturb(); }, 150);
      }
    },

    /// Set the authenticated end-user identity.
    /// Call after login: atdot.identify('user-uuid-or-any-id')
    identify: function (userId) {
      this._userId = typeof userId === 'string' && userId ? userId : null;
    },

    track: function (eventType, payload) {
      var now = Date.now();
      this._queue.push({
        session_id:  this._sessionId,
        visitor_id:  this._visitorId,
        user_id:     this._userId,
        event_type:  eventType,
        payload:     Object.assign({ pause_ms: now - this._lastTs }, payload || {}),
        timezone:    this._timezone,
        fingerprint: this._fingerprint,
        webrtc_ip:   this._webrtc ? this._webrtc.webrtc_ip : null,
        ipv6:        this._webrtc ? this._webrtc.ipv6       : null,
      });
      this._lastTs = now;
    },

    _flush: function () {
      if (!this._queue.length) return;
      var batch = this._queue.splice(0);

      // Single request for the whole batch — avoids N parallel connections
      fetch(ENDPOINT + '/api/ingest/batch', {
        method:    'POST',
        headers:   { 'Content-Type': 'application/json', 'X-API-Key': atdot._key },
        body:      JSON.stringify({ events: batch }),
        keepalive: true,
      })
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (data) {
        if (!data || !data.responses) return;
        data.responses.forEach(function (r) {
          if (r && r.action === 'challenge' && r.challenge) {
            atdot._showChallenge(r.challenge);
          }
        });
      })
      .catch(function () {});
    },

    // ── DOM perturbation ──────────────────────────────────────────────────

    _perturbSubtree: function (root) {
      var self = this;
      var AUTOMATION_ATTRS = ['data-testid', 'data-cy', 'data-test', 'data-qa', 'data-e2e'];
      var KEYWORD_IDS = [
        'submit','pay','login','signin','signup','register',
        'checkout','confirm','continue','next','proceed','buy',
        'order','place-order','complete','finish',
      ];

      function nonce() { return Math.random().toString(36).substr(2, 9); }

      function attachDecoy(real, nc, origId) {
        if (!real.parentNode) return;
        var tag   = real.tagName.toLowerCase();
        var decoy = document.createElement(tag === 'input' ? 'button' : tag);
        decoy.setAttribute('type', 'button');
        decoy.setAttribute('data-fp-decoy', nc);
        decoy.setAttribute('aria-hidden',   'true');
        decoy.setAttribute('tabindex',      '-1');
        if (origId) decoy.id = origId;
        AUTOMATION_ATTRS.forEach(function (a) {
          var v = real.getAttribute(a);
          if (v) { decoy.setAttribute(a, v); real.removeAttribute(a); }
        });
        if (real.textContent) decoy.textContent = real.textContent.trim().slice(0, 40);
        real.parentNode.insertBefore(decoy, real);
        decoy.addEventListener('click', function (e) {
          e.preventDefault(); e.stopPropagation();
          self.track('decoy_interaction', { nonce: nc, orig_id: origId || null });
        });
      }

      function processEl(el) {
        if (el.getAttribute('data-fp') || el.getAttribute('data-fp-decoy')) return;
        var nc = nonce(), origId = el.id || null;
        if (el.id) el.removeAttribute('id');
        el.setAttribute('data-fp', nc);
        attachDecoy(el, nc, origId);
      }

      var ctx    = (root === document || root === document.body) ? document : root;
      var qFn    = function (sel) {
        try { return ctx.querySelectorAll ? ctx.querySelectorAll(sel) : []; }
        catch (e) { return []; }
      };

      [].forEach.call(qFn('form'), function (form) {
        [].forEach.call(
          form.querySelectorAll('[type="submit"],button:not([type="button"]):not([type="reset"])'),
          function (btn) { processEl(btn); }
        );
      });

      KEYWORD_IDS.forEach(function (kw) {
        ['#' + kw, '#' + kw + '-btn', '#' + kw + 'Btn', '#' + kw + '_btn'].forEach(function (sel) {
          try {
            [].forEach.call(document.querySelectorAll(sel), function (el) {
              if (!el.closest('form')) processEl(el);
            });
          } catch (e) {}
        });
      });

      AUTOMATION_ATTRS.forEach(function (a) {
        [].forEach.call(qFn('[' + a + ']'), function (el) {
          if (!el.getAttribute('data-fp') && !el.getAttribute('data-fp-decoy')) processEl(el);
        });
      });
    },

    _perturb: function () {
      var self = this;
      if (!document.getElementById('_atdot_decoy_css')) {
        var style  = document.createElement('style');
        style.id   = '_atdot_decoy_css';
        style.textContent =
          '[data-fp-decoy]{' +
            'position:fixed!important;left:-9999px!important;top:0!important;' +
            'width:120px!important;height:40px!important;' +
            'display:block!important;opacity:1!important;' +
            'background:#e0e7ff!important;border:1px solid #6366f1!important;' +
            'border-radius:4px!important;cursor:pointer!important;z-index:-99999!important;' +
          '}';
        (document.head || document.documentElement).appendChild(style);
      }
      self._perturbSubtree(document.body);

      if (typeof MutationObserver !== 'undefined') {
        var timer = null;
        new MutationObserver(function (mutations) {
          clearTimeout(timer);
          timer = setTimeout(function () {
            mutations.forEach(function (m) {
              [].forEach.call(m.addedNodes, function (node) {
                if (node.nodeType === 1) self._perturbSubtree(node);
              });
            });
          }, 80);
        }).observe(document.body, { childList: true, subtree: true });
      }
    },

    // ── challenge overlay ─────────────────────────────────────────────────

    _showChallenge: function (ch) {
      if (document.getElementById('_atdot_challenge')) return;

      var overlay  = document.createElement('div');
      overlay.id   = '_atdot_challenge';
      overlay.style.cssText =
        'position:fixed;top:0;left:0;width:100%;height:100%;z-index:2147483647;' +
        'background:rgba(0,0,0,0.55);display:flex;align-items:center;justify-content:center;' +
        'font-family:system-ui,sans-serif';

      var box       = document.createElement('div');
      box.style.cssText =
        'background:#fff;border-radius:12px;padding:32px 40px;text-align:center;' +
        'max-width:340px;width:90%;box-shadow:0 8px 32px rgba(0,0,0,0.25)';

      var title     = document.createElement('p');
      title.textContent  = 'Подтвердите, что вы человек';
      title.style.cssText = 'margin:0 0 8px;font-size:18px;font-weight:600;color:#111';

      var sub       = document.createElement('p');
      sub.textContent    = 'Нажмите на кнопку ниже, чтобы продолжить';
      sub.style.cssText   = 'margin:0 0 24px;font-size:14px;color:#555';

      var btn       = document.createElement('button');
      btn.textContent    = 'Я не робот ✓';
      btn.style.cssText  =
        'display:inline-block;padding:12px 28px;border:none;border-radius:8px;' +
        'background:#4f46e5;color:#fff;font-size:15px;cursor:pointer;' +
        'transform:translate(' + ((ch.target_x - 50) * 0.6).toFixed(0) + 'px,' +
                                 ((ch.target_y - 50) * 0.3).toFixed(0) + 'px)';

      btn.addEventListener('click', function () {
        btn.disabled       = true;
        btn.textContent    = '…';
        fetch(ENDPOINT + '/api/challenge/verify', {
          method:  'POST',
          headers: { 'Content-Type': 'application/json', 'X-API-Key': atdot._key },
          body:    JSON.stringify({ challenge_id: ch.challenge_id, session_id: atdot._sessionId }),
        })
        .then(function (r) { return r.json(); })
        .then(function (res) {
          if (res.ok) { overlay.remove(); }
          else {
            sub.textContent    = 'Не удалось подтвердить. Попробуйте ещё раз.';
            btn.disabled       = false;
            btn.textContent    = 'Я не робот ✓';
          }
        })
        .catch(function () { overlay.remove(); });
      });

      box.appendChild(title); box.appendChild(sub); box.appendChild(btn);
      overlay.appendChild(box);
      document.body.appendChild(overlay);
    },

    // ── event binding ─────────────────────────────────────────────────────

    _bind: function () {
      var self = this;

      // Honeypot: invisible field only bots fill
      (function () {
        var hp = document.createElement('input');
        hp.setAttribute('type',         'text');
        hp.setAttribute('name',         'email_confirm');
        hp.setAttribute('autocomplete', 'off');
        hp.setAttribute('tabindex',     '-1');
        hp.style.cssText =
          'position:absolute;left:-9999px;top:-9999px;width:1px;height:1px;opacity:0;pointer-events:none';
        hp.addEventListener('focus',  function () { self.track('honeypot_trigger', {}); });
        hp.addEventListener('change', function () { self.track('honeypot_trigger', {}); });
        document.body
          ? document.body.appendChild(hp)
          : document.addEventListener('DOMContentLoaded', function () { document.body.appendChild(hp); });
      })();

      // Mouse position sampling
      document.addEventListener('mousemove', function (e) {
        self._mouseBuffer.push({ x: e.clientX, y: e.clientY });
      });
      self._mouseTimer = setInterval(function () {
        self._mouseBuffer = self._mouseBuffer.slice(-30);
      }, MOUSE_SAMPLE);

      // Hover tracking
      document.addEventListener('mouseover', function (e) {
        var tgt = e.target;
        if (/^(A|BUTTON|INPUT|SELECT|TEXTAREA|LABEL)$/.test(tgt.tagName)) {
          var key = (tgt.tagName + '|' + tgt.id + '|' + tgt.className).slice(0, 60);
          self._hoverStart[key] = Date.now();
        }
      });
      document.addEventListener('mouseout', function (e) {
        var key = (e.target.tagName + '|' + e.target.id + '|' + e.target.className).slice(0, 60);
        delete self._hoverStart[key];
      });

      // mousedown: capture the oldest buffered point as approach start
      // (not the mousedown position itself — the real movement began earlier)
      document.addEventListener('mousedown', function (e) {
        self._mouseStart = self._mouseBuffer.length > 0
          ? { x: self._mouseBuffer[0].x, y: self._mouseBuffer[0].y }
          : { x: e.clientX, y: e.clientY };
      });

      // Click: collect all behavioural signals
      document.addEventListener('click', function (e) {
        var tgt     = e.target;
        var rect    = tgt.getBoundingClientRect();
        var traj    = self._mouseBuffer.slice();
        self._mouseBuffer = [];

        var rectCx  = rect.left + rect.width  / 2;
        var rectCy  = rect.top  + rect.height / 2;
        var startX  = self._mouseStart ? self._mouseStart.x : e.clientX;
        var startY  = self._mouseStart ? self._mouseStart.y : e.clientY;
        var fid     = fittsID(startX, startY, rectCx, rectCy, rect.width, rect.height);

        var elemKey = (tgt.tagName + '|' + tgt.id + '|' + tgt.className).slice(0, 60);
        var hoverMs = self._hoverStart[elemKey] ? Date.now() - self._hoverStart[elemKey] : 0;

        var isNew   = !self._seenElements[elemKey];
        self._seenElements[elemKey] = true;

        var nearTarget = traj.slice(-12);
        var microCorr  = directionChanges(nearTarget);
        var vels       = velocities(traj, MOUSE_SAMPLE);

        self.track('click', {
          tag:               tgt.tagName,
          id:                tgt.getAttribute('data-fp') || tgt.id || null,
          x:                 e.clientX,
          y:                 e.clientY,
          mouse_linearity:   linearity(traj),
          trajectory_len:    traj.length,
          target_w:          rect.width,
          target_h:          rect.height,
          target_cx:         rectCx,
          target_cy:         rectCy,
          fitts_id:          fid,
          hover_duration_ms: hoverMs,
          micro_corrections: microCorr,
          max_velocity:      vels.length ? Math.max.apply(null, vels) : 0,
          final_velocity:    vels.length ? vels[vels.length - 1]      : 0,
          is_new_element:    isNew,
        });

        self._mouseStart = null;
      });

      // Scroll: depth + velocity pattern
      var maxScroll   = 0;
      var scrollEvts  = [];   // [{y, t}] — ring buffer for velocity calculation

      document.addEventListener('scroll', function () {
        var now   = Date.now();
        var depth = (window.scrollY + window.innerHeight) / Math.max(document.body.scrollHeight, 1);
        if (depth > maxScroll) maxScroll = depth;
        scrollEvts.push({ y: window.scrollY, t: now });
        if (scrollEvts.length > 60) scrollEvts.shift();
      });

      // Tab visibility: humans switch tabs, bots typically don't
      document.addEventListener('visibilitychange', function () {
        self.track('visibility_change', {
          hidden: document.hidden,
          url:    location.href,
        });
      });

      // page_hide: flush everything, include scroll velocity pattern
      window.addEventListener('pagehide', function () {
        var scrollAvgVel = 0, scrollPauses = 0;
        if (scrollEvts.length >= 2) {
          var vsum = 0, vcnt = 0;
          for (var i = 1; i < scrollEvts.length; i++) {
            var dt = scrollEvts[i].t - scrollEvts[i - 1].t;
            var dy = Math.abs(scrollEvts[i].y - scrollEvts[i - 1].y);
            if (dt > 0) { vsum += dy / dt; vcnt++; }
            if (dt > 500) scrollPauses++;  // gap > 500ms = intentional pause
          }
          scrollAvgVel = vcnt > 0 ? vsum / vcnt : 0;
        }

        self.track('page_hide', {
          scroll_depth:        maxScroll,
          scroll_avg_velocity: scrollAvgVel,
          scroll_pauses:       scrollPauses,
          url:                 location.href,
        });
        self._flush();
        clearInterval(self._flushTimer);
        clearInterval(self._mouseTimer);
      });

      // SPA navigation
      var _push = history.pushState;
      history.pushState = function () {
        _push.apply(history, arguments);
        self.track('page_view', { url: location.href });
      };
    },
  };

  // ── auto-init from script tag ─────────────────────────────────────────────

  var scripts = document.querySelectorAll('script[data-key]');
  if (scripts.length) {
    var tag = scripts[scripts.length - 1];
    var key = tag.getAttribute('data-key');
    var ep  = tag.getAttribute('data-endpoint');
    atdot.init(key, ep ? { endpoint: ep } : {});
  }

  window.atdot = atdot;

}(window, document));
