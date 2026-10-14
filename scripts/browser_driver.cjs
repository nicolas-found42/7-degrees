// Browser protocol plumbing only. Cases and assertions are owned by Rust tests.
const readline = require('node:readline');
const path = require('node:path');
const fs = require('node:fs');
let playwright;
try { playwright = require('playwright'); } catch (_) {
  playwright = require(process.env.PLAYWRIGHT_PACKAGE_PATH || path.join(process.env.HOME, '.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright'));
}
function chromiumExecutable() {
  if (process.env.BROWSER_EXECUTABLE) return process.env.BROWSER_EXECUTABLE;
  const bundled = playwright.chromium.executablePath();
  if (fs.existsSync(bundled)) return bundled;
  const cache = path.join(process.env.HOME, 'Library/Caches/ms-playwright');
  const installed = fs.readdirSync(cache).filter(n=>/^chromium_headless_shell-\d+$/.test(n)).sort((a,b)=>Number(b.split('-').pop())-Number(a.split('-').pop()));
  for (const name of installed) {
    const executable = path.join(cache,name,`chrome-headless-shell-mac-${process.arch==='arm64'?'arm64':'x64'}`,'chrome-headless-shell');
    if (fs.existsSync(executable)) return executable;
  }
  return bundled;
}
let browser, page, cdp;
let captureResponses = false;
const errors = [];
const captureErrors = [];
const responses = [], captures = [];
function observe(surface) {
  surface.on('pageerror', e => errors.push(e.message));
  surface.on('console', e => { if(e.type()==='error') errors.push(e.text()); });
  surface.on('response', response => {
    if(response.status() >= 400) errors.push(`HTTP ${response.status()} ${response.url()}`);
    if(captureResponses && (response.status() < 300 || response.status() >= 400))
      captures.push(response.body().then(body => responses.push(body)).catch(e => captureErrors.push(String(e))));
  });
}
async function command(c) {
  const target = () => { const surface = c.frame ? page.frameLocator(c.frame) : page; return c.role ? surface.getByRole(c.role, {name:c.name, exact:true}) : surface.locator(c.selector); };
  switch (c.op) {
    case 'launch':
      captureResponses = c.captureResponses === true;
      browser = await playwright.chromium.launch({headless:true, executablePath:chromiumExecutable()});
      page = await browser.newPage({viewport:{width:1200,height:1000},hasTouch:c.touch===true,deviceScaleFactor:c.dpr||1});
      observe(page);
      return browser.version();
    case 'goto': await page.goto(c.url); return page.url();
    case 'viewport': await page.setViewportSize({width:c.width,height:c.height}); return true;
    case 'ready': await target().waitFor({state:'visible'}); return true;
    case 'click': await target().click(); return true;
    case 'fill': await target().fill(c.value); return true;
    case 'open-link-modified': {
      const [popup]=await Promise.all([page.context().waitForEvent('page'),target().click({modifiers:[process.platform==='darwin'?'Meta':'Control']})]);
      await popup.waitForLoadState();const url=popup.url();await popup.close();return url;
    }
    case 'active-id': return page.evaluate(()=>document.activeElement?.id||document.activeElement?.getAttribute('data-center-player')||document.activeElement?.tagName);
    case 'open-link-text': {
      let clicked = false;
      // Native keyboard activation keeps link navigation independent of prior canvas pointer gestures.
      await target().focus();
      try {
        const [popup] = await Promise.all([page.context().waitForEvent('page'), target().press('Enter').then(()=>{clicked=true;})]);
        observe(popup); await popup.waitForLoadState();
        const text = await popup.locator('main').textContent();
        if(c.path) await popup.screenshot({path:c.path,fullPage:true});
        await popup.close(); return text;
      } catch(error) {
        const link = await target().evaluate(e=>({href:e.href,target:e.target,html:e.outerHTML}));
        let screenshot = 'tab-failure.png';
        try {
          const failurePath = path.join(__dirname,'../target/browser-e2e/tab-failure.png');
          fs.mkdirSync(path.dirname(failurePath), {recursive:true});
          await page.screenshot({path:failurePath,fullPage:true});
          screenshot = failurePath;
        } catch(screenshotError) { screenshot = `unavailable (${screenshotError})`; }
        throw new Error(`${error}; clicked=${clicked}; link=${JSON.stringify(link)}; pages=${JSON.stringify(page.context().pages().map(p=>p.url()))}; screenshot=${screenshot}`);
      }
    }
    case 'select': {
      if (await target().evaluate(e=>e.tagName==='SELECT')) { await target().selectOption(c.value); return true; }
      const name=await page.locator('#graph-payload').evaluate((e,id)=>JSON.parse(e.textContent).nodes.find(n=>n.id===id)?.name,c.value);
      if(!name) throw new Error('Unknown player '+c.value);
      await target().fill(name);
      await page.locator(c.selector+'-results [data-player-id="'+c.value+'"]').click();
      return true;
    }
    case 'input-value': return target().inputValue();
    case 'history': if(c.direction==='back') await page.goBack(); else await page.goForward(); return page.url();
    case 'url': return page.url();
    case 'motion': await page.emulateMedia({reducedMotion:c.value}); return true;
    case 'page-size': return page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth}));
    case 'sequence': for(const selector of c.selectors) await page.locator(selector).click({force:true}); return true;
    case 'observe-status': await page.evaluate(()=>{window.statusAnnouncements=[];new MutationObserver(()=>window.statusAnnouncements.push(document.querySelector('#canvas-status').textContent)).observe(document.querySelector('#canvas-status'),{childList:true,subtree:true});}); return true;
    case 'status-announcements': return page.evaluate(()=>window.statusAnnouncements);
    case 'settled': await page.waitForFunction(()=>document.querySelector('#canvas-status')?.dataset.transition!=='running'); return true;
    case 'touch': if(!cdp) cdp=await page.context().newCDPSession(page); await cdp.send('Input.dispatchTouchEvent',{type:c.type,touchPoints:c.points||[]}); return true;
    case 'pointer': await target().dispatchEvent(c.event,{pointerId:c.id,clientX:c.x,clientY:c.y,pointerType:'touch',button:0,buttons:1}); return true;
    case 'intercept': {
      await page.route(c.pattern,async route=>{
        if(c.delay) await new Promise(resolve=>setTimeout(resolve,c.delay));
        if(c.fail) await route.abort(); else await route.continue();
      }); return true;
    }
    case 'unroute': await page.unroute(c.pattern); return true;
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
    case 'response-contains':
      if (!captureResponses) throw new Error('Launch with captureResponses:true before testing response contents');
      await page.waitForLoadState('networkidle'); await Promise.all(captures); {
      const found = responses.some(body => body.includes(Buffer.from(c.text)));
      if (!found && captureErrors.length) throw new Error('Response capture incomplete: ' + captureErrors.join('\n'));
      return found;
    }
    case 'network-idle': await page.waitForLoadState('networkidle'); return true;
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
