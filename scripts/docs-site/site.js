// The docs site's few behaviors: the theme toggle (Auto, Light, Dark,
// remembered in localStorage), the sidebar on narrow screens, search over
// page titles and headings (assets/search-index.js), and copy buttons on code.
(function () {
  'use strict';
  var root = document.body.getAttribute('data-root') || '';

  // Theme: Auto follows the system; Light and Dark set data-theme on <html>.
  var themes = ['auto', 'light', 'dark'];
  var themeButton = document.querySelector('.theme-button');
  function currentTheme() {
    var t = document.documentElement.getAttribute('data-theme');
    return t === 'light' || t === 'dark' ? t : 'auto';
  }
  function showTheme() {
    var t = currentTheme();
    themeButton.textContent = t[0].toUpperCase() + t.slice(1);
    themeButton.setAttribute('aria-label', 'Theme: ' + t + ' (change)');
  }
  if (themeButton) {
    showTheme();
    themeButton.addEventListener('click', function () {
      var next = themes[(themes.indexOf(currentTheme()) + 1) % themes.length];
      if (next === 'auto') document.documentElement.removeAttribute('data-theme');
      else document.documentElement.setAttribute('data-theme', next);
      try {
        if (next === 'auto') localStorage.removeItem('docs-theme');
        else localStorage.setItem('docs-theme', next);
      } catch (e) {
        // Storage can be blocked; the choice then lasts for this page.
      }
      showTheme();
    });
  }

  // The sidebar is a panel behind the Menu button on narrow screens.
  var menuButton = document.querySelector('.menu-button');
  var sidebar = document.getElementById('sidebar');
  function setMenu(open) {
    sidebar.classList.toggle('open', open);
    menuButton.setAttribute('aria-expanded', String(open));
    menuButton.textContent = open ? 'Close' : 'Menu';
  }
  if (menuButton && sidebar) {
    menuButton.addEventListener('click', function () {
      setMenu(!sidebar.classList.contains('open'));
    });
    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && sidebar.classList.contains('open')) {
        setMenu(false);
        menuButton.focus();
      }
    });
  }

  // Search: titles and headings containing the query, pages first.
  var input = document.getElementById('search-input');
  var results = document.getElementById('search-results');
  var index = window.DOCS_INDEX || [];
  var shown = [];
  var active = -1;
  function close() {
    results.hidden = true;
    input.setAttribute('aria-expanded', 'false');
    input.removeAttribute('aria-activedescendant');
    active = -1;
  }
  function setActive(i) {
    var items = results.querySelectorAll('li[role="option"]');
    items.forEach(function (li, k) {
      li.setAttribute('aria-selected', String(k === i));
    });
    active = i;
    if (i >= 0 && items[i]) {
      input.setAttribute('aria-activedescendant', items[i].id);
      items[i].scrollIntoView({ block: 'nearest' });
    }
  }
  function search() {
    var q = input.value.trim().toLowerCase();
    if (!q) return close();
    var words = q.split(/\s+/);
    shown = index
      .filter(function (e) {
        var t = e.t.toLowerCase();
        return words.every(function (w) {
          return t.indexOf(w) >= 0;
        });
      })
      .map(function (e) {
        var t = e.t.toLowerCase();
        var page = e.u.indexOf('#') < 0;
        return { e: e, score: (t === q ? 0 : t.indexOf(q) === 0 ? 1 : 2) + (page ? 0 : 0.5) + t.length / 1000 };
      })
      .sort(function (a, b) {
        return a.score - b.score;
      })
      .slice(0, 12)
      .map(function (r) {
        return r.e;
      });
    results.innerHTML = '';
    if (!shown.length) {
      var none = document.createElement('li');
      none.className = 'empty';
      none.textContent = 'Nothing found';
      results.appendChild(none);
    }
    shown.forEach(function (e, k) {
      var li = document.createElement('li');
      li.id = 'search-result-' + k;
      li.setAttribute('role', 'option');
      var a = document.createElement('a');
      a.href = root + e.u;
      a.tabIndex = -1;
      a.textContent = e.t;
      var small = document.createElement('small');
      small.textContent = e.p;
      a.appendChild(small);
      li.appendChild(a);
      results.appendChild(li);
    });
    results.hidden = false;
    input.setAttribute('aria-expanded', 'true');
    setActive(shown.length ? 0 : -1);
  }
  if (input && results) {
    input.addEventListener('input', search);
    input.addEventListener('focus', function () {
      if (input.value.trim()) search();
    });
    input.addEventListener('keydown', function (e) {
      if (e.key === 'ArrowDown' && shown.length) {
        e.preventDefault();
        setActive((active + 1) % shown.length);
      } else if (e.key === 'ArrowUp' && shown.length) {
        e.preventDefault();
        setActive((active - 1 + shown.length) % shown.length);
      } else if (e.key === 'Enter' && active >= 0 && shown[active]) {
        e.preventDefault();
        window.location.href = root + shown[active].u;
      } else if (e.key === 'Escape') {
        input.value = '';
        close();
      }
    });
    document.addEventListener('click', function (e) {
      if (!e.target.closest || !e.target.closest('.search')) close();
    });
    document.addEventListener('keydown', function (e) {
      var tag = (document.activeElement && document.activeElement.tagName) || '';
      if (e.key === '/' && tag !== 'INPUT' && tag !== 'TEXTAREA' && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        input.focus();
      }
    });
  }

  // Copy buttons on code blocks.
  if (navigator.clipboard) {
    document.querySelectorAll('.doc pre').forEach(function (pre) {
      var code = pre.querySelector('code');
      if (!code) return;
      var button = document.createElement('button');
      button.type = 'button';
      button.className = 'copy-button';
      button.textContent = 'Copy';
      button.setAttribute('aria-label', 'Copy code');
      button.addEventListener('click', function () {
        navigator.clipboard.writeText(code.textContent.replace(/\n$/, '')).then(
          function () {
            button.textContent = 'Copied';
            setTimeout(function () {
              button.textContent = 'Copy';
            }, 1500);
          },
          function () {
            button.textContent = 'Press ⌘C';
          },
        );
      });
      pre.appendChild(button);
    });
  }
})();
