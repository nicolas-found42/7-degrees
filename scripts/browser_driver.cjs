// Browser protocol plumbing only. Cases and assertions are owned by Rust tests.
const readline = require('node:readline');
const path = require('node:path');
let playwright;
try { playwright = require('playwright'); } catch (_) {
  playwright = require(process.env.PLAYWRIGHT_PACKAGE_PATH || path.join(process.env.HOME, '.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright'));
}
let browser, page;
const errors = [];
const captureErrors = [];
const responses = [], captures = [];
function observe(surface) {
  surface.on('pageerror', e => errors.push(e.message));
  surface.on('console', e => { if(e.type()==='error') errors.push(e.text()); });
  surface.on('response', response => {
    if(response.status() < 300 || response.status() >= 400)
      captures.push(response.body().then(body => responses.push(body)).catch(e => captureErrors.push(String(e))));
  });
}
async function command(c) {
  const target = () => { const surface = c.frame ? page.frameLocator(c.frame) : page; return c.role ? surface.getByRole(c.role, {name:c.name, exact:true}) : surface.locator(c.selector); };
  switch (c.op) {
    case 'launch':
      browser = await playwright.chromium.launch({headless:true, executablePath:process.env.BROWSER_EXECUTABLE || path.join(process.env.HOME, 'Library/Caches/ms-playwright/chromium_headless_shell-1243/chrome-headless-shell-mac-arm64/chrome-headless-shell')});
      page = await browser.newPage({viewport:{width:1200,height:1000}});
      observe(page);
      return browser.version();
    case 'goto': await page.goto(c.url); return page.url();
    case 'viewport': await page.setViewportSize({width:c.width,height:c.height}); return true;
    case 'ready': await target().waitFor({state:'visible'}); return true;
    case 'click': await target().click(); return true;
    case 'fill': await target().fill(c.value); return true;
    case 'open-link-text': { const [popup] = await Promise.all([page.waitForEvent('popup'), target().click()]); observe(popup); await popup.waitForLoadState(); const text = await popup.locator('main').textContent(); if(c.path) await popup.screenshot({path:c.path,fullPage:true}); await popup.close(); return text; }
    case 'select': await target().selectOption(c.value); return true;
    case 'attribute': return target().getAttribute(c.name);
    case 'text': return target().textContent();
    case 'count': return target().count();
    case 'box': return target().boundingBox();
    case 'payload-counts': return page.locator('#graph-payload').evaluate(element => {
      const payload = JSON.parse(element.getAttribute('data-payload') || element.textContent);
      return {players:payload.nodes.length, relationships:payload.links.length, full_network:payload.full_network, truncated:payload.truncated, path:payload.path, highlighted:payload.links.filter(e=>e.on_path).length};
    });
    case 'focus': await target().focus(); return true;
    case 'key': await page.keyboard.press(c.key); return true;
    case 'mouse':
      if(c.action==='move') await page.mouse.move(c.x,c.y,{steps:c.steps||1});
      if(c.action==='down') await page.mouse.down();
      if(c.action==='up') await page.mouse.up();
      if(c.action==='click') await page.mouse.click(c.x,c.y);
      if(c.action==='wheel') await page.mouse.wheel(0,c.delta);
      return true;
    case 'canvas': return target().evaluate(canvas=>canvas.toDataURL());
    case 'screenshot': if(c.selector) await target().screenshot({path:c.path}); else await page.screenshot({path:c.path,fullPage:true}); return true;
    case 'event-start': await page.evaluate(name => { window.browserEvents = []; document.addEventListener(name, event => window.browserEvents.push(event.detail)); }, c.name); return true;
    case 'events': return page.evaluate(() => window.browserEvents);
    case 'response-contains': await page.waitForLoadState('networkidle'); await Promise.all(captures); {
      const found = responses.some(body => body.includes(Buffer.from(c.text)));
      if (!found && captureErrors.length) throw new Error('Response capture incomplete: ' + captureErrors.join('\n'));
      return found;
    }
    case 'errors': return errors;
    case 'close': await browser.close(); return true;
    default: throw new Error('unknown browser operation');
  }
}
(async()=>{
  for await(const line of readline.createInterface({input:process.stdin})) {
    let c;
    try { c=JSON.parse(line); process.stdout.write(JSON.stringify({ok:true,result:await command(c)})+'\n'); }
    catch(e) { process.stdout.write(JSON.stringify({ok:false,error:String(e)})+'\n'); }
    if(c?.op==='close') break;
  }
})().finally(async()=>{if(browser)await browser.close();});
