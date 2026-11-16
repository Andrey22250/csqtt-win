const { chromium } = require('../../../.tools/ui-tests/node_modules/playwright');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const fs = require('node:fs');
const path = require('node:path');
const artifactDir = path.resolve(__dirname, '../../../dist/verification-0.1.1');
fs.mkdirSync(artifactDir, { recursive: true });
const verify = process.argv.includes('--verify');
(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 520, height: 860 } });
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(pathToFileURL(path.resolve(__dirname, '../../../windows/csqtt-cli/gui/index.html')).href);
    await page.waitForFunction(() => window.App && App.state.settings);
    const nav = page.locator('.nav-item[data-screen-id="tunnel"]');
    const rect = await nav.boundingBox();
    await page.evaluate(() => { window.pressedNav = document.querySelector('.nav-item[data-screen-id="tunnel"]'); });
    await page.mouse.move(rect.x + rect.width / 2, rect.y + rect.height / 2);
    await page.mouse.down();
    await page.evaluate(() => __csqttHost.recv([{ event: 'status', data: { status: { ...App.state.status, bytes_down: 1234 } } }]));
    const stableNav = await page.evaluate(() => pressedNav === document.querySelector('.nav-item[data-screen-id="tunnel"]'));
    await page.mouse.up();
    const selectedAfterClick = await page.locator('.nav-item.selected').getAttribute('data-screen-id');
    await page.evaluate(() => {
      csqtt.listProcesses = () => Promise.resolve(Array.from({ length: 80 }, (_, i) => ({
        name: 'process-' + i + '.exe', path: 'C:/Program Files/' + 'VeryLongApplicationDirectory/'.repeat(5) + i + '/program.exe'
      })));
      App.showScreen('exceptions'); Exceptions.showAppsDialog();
    });
    await page.getByRole('button', { name: 'Из запущенных', exact: true }).click();
    const top = page.locator('.dialog-overlay').last();
    await top.getByText('process-0.exe', { exact: true }).waitFor();
    await page.waitForFunction(() => document.querySelector('.dialog-overlay:last-child').classList.contains('active'));
    await page.screenshot({ path: path.join(artifactDir, verify ? 'processes-fixed.png' : 'processes-before.png'), animations: 'disabled' });
    const layout = await top.locator('.dialog-content').evaluate(el => {
      const rows = [...el.querySelectorAll('button')];
      return { count: rows.length, minHeight: Math.min(...rows.map(r => r.getBoundingClientRect().height)),
        clipped: rows.some(r => r.scrollHeight > r.clientHeight + 2), overflow: el.scrollWidth > el.clientWidth + 1 };
    });
    await page.keyboard.press('Escape');
    await page.waitForTimeout(250);
    const remainingAfterEscape = await page.locator('.dialog-overlay').count();
    const diagnostics = { stableNav, selectedAfterClick, layout, remainingAfterEscape, errors };
    console.log(JSON.stringify(diagnostics, null, 2));
    fs.writeFileSync(path.join(artifactDir, verify ? 'ui-results.json' : 'ui-before.json'), JSON.stringify(diagnostics, null, 2));
    if (!verify) return;
    assert.equal(stableNav, true); assert.equal(selectedAfterClick, 'tunnel');
    assert.equal(layout.count, 80); assert.ok(layout.minHeight >= 40); assert.equal(layout.clipped, false); assert.equal(layout.overflow, false);
    assert.equal(remainingAfterEscape, 1);
    assert.equal(await page.getByRole('button', { name: 'Из запущенных', exact: true }).evaluate(el => el === document.activeElement), true);
    await page.evaluate(() => { csqtt.listProcesses = () => Promise.resolve([]); });
    await page.getByRole('button', { name: 'Из запущенных', exact: true }).click();
    await page.waitForFunction(() => !document.querySelector('.dialog-overlay:last-child .dialog-content').textContent.includes('Загрузка'));
    await page.keyboard.press('Escape'); await page.waitForTimeout(230);
    await page.evaluate(() => { csqtt.listProcesses = () => Promise.reject(new Error('test process failure')); });
    await page.getByRole('button', { name: 'Из запущенных', exact: true }).click();
    await page.getByText(/test process failure/).waitFor();
    await page.evaluate(() => { csqtt.listProcesses = () => Promise.resolve([{ name: 'test.exe', path: 'C:/test.exe' }]); });
    await page.locator('.dialog-overlay').last().getByRole('button', { name: /Повтор|Обнов/ }).click();
    await page.locator('.dialog-overlay').last().getByText('test.exe', { exact: true }).click();
    await page.waitForTimeout(250);
    assert.ok((await page.evaluate(() => App.profile().excluded_apps)).includes('C:/test.exe'));
    await page.keyboard.press('Escape'); await page.waitForTimeout(250);
    const count = await page.evaluate(async () => {
      let closed = 0; const d = UI.dialog({ title: 'Close once', onClose: () => closed++ });
      d.close(); d.close(); await new Promise(r => setTimeout(r, 250)); return closed;
    });
    assert.equal(count, 1);
    // A log badge update must also preserve a pressed button.
    assert.equal(await page.evaluate(() => {
      const before = document.querySelector('.nav-item[data-screen-id="logs"]');
      __csqttHost.recv([{ event: 'log', data: { line: '[ERR] badge regression check' } }]);
      return before === document.querySelector('.nav-item[data-screen-id="logs"]');
    }), true);
    await page.setViewportSize({ width: 360, height: 740 });
    await page.evaluate(async () => {
      await App.saveSettings(s => { s.theme_mode = 'dark'; });
      csqtt.listProcesses = () => Promise.resolve(Array.from({length: 80}, (_, i) => ({
        name: 'process-' + i + '.exe', path: 'C:/' + 'VeryLongPath/'.repeat(12) + i + '/program.exe'
      })));
    });
    await page.evaluate(() => { Exceptions.showAppsDialog(); });
    await page.getByRole('button', { name: 'Из запущенных', exact: true }).click();
    const search = page.getByRole('textbox', { name: 'Поиск запущенного процесса' });
    await search.fill('process-79');
    assert.equal(await page.locator('.dialog-overlay').last().locator('.proc-picker-row').count(), 1);
    await search.fill('no-such-process');
    await page.getByText('Ничего не найдено', { exact: true }).waitFor();
    await search.fill('');
    assert.equal(await page.locator('.dialog-overlay').last().locator('.proc-picker-row').count(), 80);
    await page.waitForFunction(() => document.querySelector('.dialog-overlay:last-child').classList.contains('active'));
    await page.screenshot({ path: path.join(artifactDir, 'processes-fixed-360.png'), animations: 'disabled' });
    assert.equal(await page.locator('.dialog-overlay').last().locator('.dialog-card').evaluate(el => el.scrollWidth > el.clientWidth + 1), false);
    assert.deepEqual(errors, []);
    console.log('PASS: nav pointer race, 80 process rows, empty/error/retry, selection, nested Escape/focus, idempotent close, badge updates, narrow layout');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
