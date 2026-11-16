# Проверки интерфейса

Требуются Node.js и установленный Microsoft Edge. Из корня проекта:

```powershell
npm install --prefix .tools/ui-tests --ignore-scripts --no-audit --no-fund playwright@1.65.1
node scripts/windows/tests/routing-regression.cjs
node scripts/windows/tests/ui-regression.cjs --verify
```

Проверки используют mock-бэкенд страницы, не подключаются к службе и не меняют рабочий VPN. Снимки и результаты записываются в `dist/verification-0.1.1/`. Зависимости в `.tools` и результаты в `dist` не добавляются в Git.

Проверка UI без `--verify` сохраняет диагностические данные, но не проверяет утверждения регрессионного теста.
