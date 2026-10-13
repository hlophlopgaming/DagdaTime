import {useEffect,useRef} from 'react';

// The window usually lives hidden in the tray. Polling a hidden window wastes CPU and
// battery, so timers stop while hidden and refresh once when the window comes back.
// Tauri hides the window instead of closing it; the backend emits `window-visibility`
// because not every WebView reports that through the Page Visibility API.
let hidden=typeof document!=='undefined'&&document.visibilityState==='hidden';
const listeners=new Set<()=>void>();
export function setHidden(value:boolean){if(hidden!==value){hidden=value;listeners.forEach(listener=>listener());}}
export function isHidden(){return hidden;}
if(typeof document!=='undefined')document.addEventListener('visibilitychange',()=>setHidden(document.visibilityState==='hidden'));
if(typeof window!=='undefined')window.__TAURI__?.event?.listen('window-visibility',event=>setHidden(event.payload===false));

/** Call `load` every `interval` ms while the window is visible and `enabled` is true. */
export function usePolling(load:()=>void,interval:number,enabled=true){
  const latest=useRef(load);latest.current=load;
  useEffect(()=>{
    if(!enabled)return;
    let timer:ReturnType<typeof setInterval>|undefined;
    const schedule=()=>{if(timer!==undefined)clearInterval(timer);timer=hidden?undefined:setInterval(()=>latest.current(),interval);};
    const changed=()=>{if(!hidden)latest.current();schedule();};
    listeners.add(changed);schedule();
    return()=>{listeners.delete(changed);if(timer!==undefined)clearInterval(timer);};
  },[interval,enabled]);
}

/** Exact comparison includes edits to titles, tag metadata and range boundaries. */
export function fingerprint(...lists:unknown[][]){
  return JSON.stringify(lists);
}
