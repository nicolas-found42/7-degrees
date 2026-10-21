// Exercise the prototype's own BFS and layout against the served snapshot.
// Run while scripts/prototype-fluid.py is serving: node <this file>
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const html=readFileSync(new URL('./visual-directions.html',import.meta.url),'utf8');
const script=html.split('<script>')[1].split('</script>')[0];
new vm.Script(script); // Check the complete browser script, including preview handling.
const data=await (await fetch('http://127.0.0.1:4317/api/network')).json();
let viewport={width:1164,height:876};
const els=new Map();
const el=id=>{if(!els.has(id))els.set(id,{value:'',style:{},get clientWidth(){return viewport.width},get clientHeight(){return viewport.height}});return els.get(id)};
const ctx=vm.createContext({document:{getElementById:el},location:{search:'?variant=J'},URLSearchParams,innerWidth:1440,matchMedia:()=>({matches:false}),console,data});
vm.runInContext(script.slice(script.indexOf('const $='),script.indexOf("$('pair-form').onsubmit")),ctx);
vm.runInContext(`nodes=data.nodes;links=data.links;nodes.forEach((n,i)=>byId.set(n.id,i));adj=nodes.map(()=>[]);for(const e of links){const a=byId.get(e.from),b=byId.get(e.to);adj[a].push(b);adj[b].push(a);teams.set(edgeKey(a,b),[...(e.franchises?.length?e.franchises:[e.team])].sort());relationships.set(edgeKey(a,b),e);}adj=adj.map(a=>[...new Set(a)]);update=()=>derive();`,ctx);
const run=code=>vm.runInContext(code,ctx);
const pair=(a,b)=>{ctx.startName=a;ctx.endName=b;run('findPair(nodes.findIndex(n=>n.name===startName),nodes.findIndex(n=>n.name===endName))')};
const snapshot=()=>JSON.parse(run(`JSON.stringify({minimum:distance,chain:chain.map(i=>nodes[i].name),eligible:eligible.map(i=>nodes[i].name),total:adj[chain.at(-1)].length})`));
for(const [width,height]of [[1440,900],[390,844]]){
 ctx.innerWidth=width;viewport={width:width===390?390:1164,height:height===844?432:876};
 pair('Michael Jordan','Stephen Curry');assert.equal(snapshot().minimum,3);assert.equal(snapshot().chain.length,4);
 run('targets=targetsForScene()');const xs=JSON.parse(run('JSON.stringify(chain.map(i=>targets.get(i).x))'));assert(xs.every((x,k)=>!k||x>xs[k-1]));
 run('chain=[root];derive();targets=targetsForScene()');assert.equal(snapshot().eligible.length,28);assert.equal(snapshot().total,102);
 const boxes=JSON.parse(run('JSON.stringify(eligible.map(i=>({...targets.get(i),name:nodes[i].name})))'));
 for(let a=0;a<boxes.length;a++)for(let b=a+1;b<boxes.length;b++)assert(Math.abs(boxes[a].x-boxes[b].x)>=148||Math.abs(boxes[a].y-boxes[b].y)>=32,`${width}: overlapping pills`);
 for(const p of boxes){assert(p.x>=74);assert(p.x+74<=run('sceneWidth'));}
 run(`extend(nodes.findIndex(n=>n.name==='Scottie Pippen'));targets=targetsForScene();pins.set(root,clampPoint(root,{x:9999,y:-9999}));`);
 const pinned=JSON.parse(run('JSON.stringify(pins.get(root))'));assert(pinned.y>=(width===390?80:205));assert(pinned.x<run('targets.get(chain.at(-1)).x'));
 run('reveal();extend(chain[1]);targets=targetsForScene()');assert.equal(snapshot().chain.length,2);
 const endBox=JSON.parse(run('JSON.stringify(targets.get(end))'));assert(Math.abs(endBox.x-pinned.x)>180||Math.abs(endBox.y-pinned.y)>75);
 pair('Michael Jordan','LeBron James');assert(snapshot().minimum>0);
 pair('Michael Jordan','Michael Jordan');assert.equal(snapshot().minimum,0);assert.equal(snapshot().eligible.length,0);
 pair('Michael Jordan','Timmy Allen');assert.equal(snapshot().minimum,-1);assert.equal(run('adj[end].length'),0);run('targets=targetsForScene()');assert(run('targets.has(end)'));
 console.log(`PASS ${width} × ${height}: completed route, 28 distinct pills / 102 total, advance, drag bounds, rewind destination, changed destination, same player, isolated target`);
}
assert(run('links.every(e=>team(byId.get(e.from),byId.get(e.to))===e.team)'),'Franchise labels must match the backend overlap evidence team, including multi-franchise pairs');
assert(html.includes('Context edges'));assert(html.includes('minimum_shared_games'));assert(html.includes('0 evidenced connections in snapshot'));assert(html.includes("$('hint').hidden=distance<=0"));
console.log(`PASS snapshot: ${data.nodes.length} players / ${data.links.length} edges; prototype script parses`);
