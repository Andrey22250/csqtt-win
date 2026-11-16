const { chromium } = require('../../../.tools/ui-tests/node_modules/playwright');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');
const fs = require('node:fs');
const artifactDir = path.resolve(__dirname, '../../../dist/verification-0.1.1');
fs.mkdirSync(artifactDir, { recursive: true });

(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  const results = [];
  try {
    const page = await browser.newPage({ viewport: { width: 520, height: 860 } });
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(pathToFileURL(path.resolve(__dirname, '../../../windows/csqtt-cli/gui/index.html')).href);
    await page.waitForFunction(() => window.App && App.state.settings);

    const saved = await page.evaluate(async () => {
      const originalCall = csqttMock.call;
      csqttMock.call = function(method, params, emit, id) {
        // Real overlapping requests reliably exposed the old get/get/set/set race.
        if (method === 'ipc' && /^(get_settings|set_settings)$/.test(params.request.cmd)) {
          return new Promise((resolve, reject) => setTimeout(() => {
            originalCall(method, params, emit, id).then(resolve, reject);
          }, 20));
        }
        return originalCall(method, params, emit, id);
      };
      await Promise.all([
        App.saveSettings(s => { s.profiles[0].template_domains_enabled = true; }),
        App.saveSettings(s => { s.profiles[0].exclude_ru_zones_enabled = true; }),
        App.saveSettings(s => { s.profiles[0].direct_domains = 'selectel.ru,!example.ru'; }),
        App.saveSettings(s => { s.theme_mode = 'dark'; })
      ]);
      try { await csqtt.updateSettings(() => { throw new Error('injected save failure'); }); } catch {}
      await App.saveSettings(s => { s.profiles[0].sni = 'test.example'; });
      return csqtt.getSettings();
    });
    assert.equal(saved.profiles[0].template_domains_enabled, true);
    assert.equal(saved.profiles[0].exclude_ru_zones_enabled, true);
    assert.equal(saved.profiles[0].direct_domains, 'selectel.ru,!example.ru');
    assert.equal(saved.profiles[0].sni, 'test.example');
    assert.equal(saved.theme_mode, 'dark');
    results.push('Overlapping saves preserve every field; queue recovers after a failed mutator');

    await page.evaluate(() => { App.showScreen('exceptions'); Exceptions.showDomainsDialog(); });
    const ownDomains = page.getByRole('textbox', { name: 'Свои домены', exact: true });
    const exceptions = page.getByRole('textbox', { name: 'Исключения (всегда через VPN)', exact: true });
    assert.equal(await ownDomains.inputValue(), 'selectel.ru');
    assert.equal(await exceptions.inputValue(), 'example.ru');
    const template = page.locator('.setting-row').filter({ hasText: 'Шаблон РФ-сервисов' });
    await template.click();
    await template.click();
    assert.equal(await ownDomains.inputValue(), 'selectel.ru');
    await exceptions.fill('example.ru, !https://www.Yandex.ru:443/path');
    await page.getByRole('button', { name: 'Белый список', exact: true }).click();
    await page.getByRole('button', { name: 'Чёрный список', exact: true }).click();
    await page.getByRole('button', { name: 'Готово', exact: true }).click();
    await page.waitForSelector('.dialog-overlay', { state: 'detached' });
    const afterDialog = await page.evaluate(() => csqtt.getSettings());
    const profile = afterDialog.profiles[0];
    assert.equal(profile.template_domains_enabled, true);
    assert.equal(profile.exclude_ru_zones_enabled, true);
    assert.equal(profile.domain_is_whitelist, false);
    assert.equal(profile.direct_domains_black, '!example.ru,!yandex.ru,selectel.ru');
    assert.equal(profile.direct_domains_white, '!example.ru,!yandex.ru');
    assert.equal(profile.direct_domains, profile.direct_domains_black);
    results.push('Legacy domain list loads; template/mode toggles keep custom domains, flags and VPN exceptions');

    await page.evaluate(() => Exceptions.showDomainsDialog());
    assert.equal(await exceptions.inputValue(), 'example.ru, yandex.ru');
    await page.screenshot({ path: path.join(artifactDir, 'routing-dark-520.png'), fullPage: true, animations: 'disabled' });
    const overflow = await page.evaluate(() => Array.from(document.querySelectorAll('.dialog-card,.dialog-content'))
      .some(el => el.scrollWidth > el.clientWidth + 1));
    assert.equal(overflow, false);
    await page.setViewportSize({ width: 360, height: 740 });
    await page.screenshot({ path: path.join(artifactDir, 'routing-dark-360.png'), fullPage: true, animations: 'disabled' });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    assert.equal(await page.getByRole('button', { name: 'Готово', exact: true }).isVisible(), true);
    await page.evaluate(() => App.saveSettings(s => { s.theme_mode = 'light'; }));
    await page.setViewportSize({ width: 520, height: 860 });
    await page.screenshot({ path: path.join(artifactDir, 'routing-light-520.png'), fullPage: true, animations: 'disabled' });
    await ownDomains.fill('');
    await exceptions.fill('');
    await page.getByRole('button', { name: 'Готово', exact: true }).click();
    await page.waitForSelector('.dialog-overlay', { state: 'detached' });
    const cleared = await page.evaluate(() => csqtt.getSettings());
    assert.equal(cleared.profiles[0].direct_domains, '');
    assert.equal(cleared.profiles[0].direct_domains_black, '');
    results.push('Reopening keeps exceptions; clearing removes legacy fallback; 360/520px layouts have no horizontal overflow');

    await page.evaluate(() => { App.showScreen('connection'); });
    const sendStatus = async (state, start) => page.evaluate(({ state, start }) => {
      const status = csqttMock.getStatus();
      status.state = state;
      status.connected_since_ms = start;
      __csqttHost.recv([{ event: 'status', data: { status: { ...status } } }]);
    }, { state, start });
    const initialStart = await page.evaluate(() => Date.now() - 125000);
    await sendStatus('connected', initialStart);
    assert.match(await page.locator('.connect-uptime-text').innerText(), /^00:02:0[5-9]$/);
    // Simulate a suspended/throttled window: one wake-up must account for the whole gap.
    await page.evaluate(() => { window.realNow = Date.now; Date.now = () => realNow() + 60000; });
    await page.waitForFunction(() => document.querySelector('.connect-uptime-text').textContent.startsWith('00:03:'));
    await sendStatus('failed', null);
    assert.equal(await page.locator('.connect-uptime-text').innerText(), '00:00:00');
    await sendStatus('connected', await page.evaluate(() => Date.now() - 4000));
    assert.match(await page.locator('.connect-uptime-text').innerText(), /^00:00:0[4-6]$/);
    await sendStatus('reconnecting', null);
    assert.equal(await page.locator('.connect-uptime-text').innerText(), '00:00:00');
    results.push('Timer restores service age, catches up after a time gap, resets on failure/reconnect and uses the new connection start');
    assert.deepEqual(errors, []);
    fs.writeFileSync(path.join(artifactDir, 'results.json'), JSON.stringify({ passed: results, pageErrors: errors }, null, 2));
    console.log(results.join('\n'));
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
