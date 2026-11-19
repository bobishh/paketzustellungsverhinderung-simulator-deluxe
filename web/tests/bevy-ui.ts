import {expect,type Page} from '@playwright/test';
export type Region={x:number;y:number;width:number;height:number;label?:string;action?:string;capture?:boolean};
export async function observeGame(page:Page){await page.addInitScript(()=>{
 const facts:Record<string,unknown>={};(globalThis as any).paketObserved=facts;
 for(const name of ['paket-navigation','paket-mode','paket-lobby','paket-hud','paket-radar','paket-members','paket-session','paket-notice','paket-map-loading','paket-ride-loading','paket-ui-regions','paket-services','paket-profile'])window.addEventListener(name,e=>{
  const value=(e as CustomEvent).detail;
  if(name==='paket-mode'&&value!=='ride')delete facts['paket-hud'];
  facts[name]=value;
 });
});}
export async function fact<T=any>(page:Page,name:string):Promise<T>{return page.evaluate(name=>(globalThis as any).paketObserved?.[name],name);}
export async function clickUi(page:Page,label:string|RegExp){
 await expect(page.locator('#bootstrap')).toBeHidden({timeout:90_000});
 let region:Region|undefined;
 await expect.poll(async()=>{const data=await fact<{regions:Region[]}>(page,'paket-ui-regions');region=data?.regions.find(r=>typeof label==='string'?r.label===label:label.test(r.label??''));return Boolean(region);},{timeout:90_000}).toBe(true);
 const box=await page.locator('#game').boundingBox();if(!box||!region)throw new Error(`Game control unavailable: ${label}`);
 // Let the actual picking backend see the pointer before its press/release.
 await page.mouse.move(box.x+region.x+region.width/2,box.y+region.y+region.height/2);
 await page.evaluate(()=>new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve()))));
 await page.mouse.click(box.x+region.x+region.width/2,box.y+region.y+region.height/2);
 await page.evaluate(()=>new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve()))));
}
export async function waitRide(page:Page){await expect(page.locator('#bootstrap')).toBeHidden({timeout:90_000});await expect.poll(async()=>(await fact(page,'paket-mode'))==='ride'&&Boolean((await fact(page,'paket-hud'))?.ready),{timeout:90_000}).toBe(true);}
export async function hud(page:Page){return await fact(page,'paket-hud')??{};}
export async function selected(page:Page){const h=await hud(page);return h.routes?.[h.selected];}
export async function fillUi(page:Page,label:string,text:string){await clickUi(page,label);await page.keyboard.press('ControlOrMeta+A');await page.keyboard.type(text);}
