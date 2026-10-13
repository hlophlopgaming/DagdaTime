import {useEffect,useState} from 'react';
import {invoke} from './client.ts';
import {appColor,applicationLabel} from './timeline.ts';
const cache=new Map<string,Promise<string|null>>();
// Icons requested in the same render (a summary table mounts dozens at once) are
// resolved with one native call instead of one IPC round trip per row.
let pending=new Map<string,(icon:string|null)=>void>();
function flush(){
  const batch=pending;pending=new Map();
  invoke<Record<string,string|null>>('application_icons',{apps:[...batch.keys()]})
    .then(value=>batch.forEach((resolve,app)=>resolve(value?.[app]??null)))
    .catch(()=>batch.forEach(resolve=>resolve(null)));
}
function load(app:string){
  return new Promise<string|null>(resolve=>{if(!pending.size)queueMicrotask(flush);pending.set(app,resolve);});
}
export default function AppIcon({app}:{app:string}){
  const [source,setSource]=useState<string|null>(null);
  useEffect(()=>{
    let alive=true;setSource(null);
    if(!cache.has(app))cache.set(app,load(app));
    cache.get(app)!.then(value=>{if(alive)setSource(value);});
    return()=>{alive=false;};
  },[app]);
  return source?<img className="app-icon" src={source} alt={`${applicationLabel(app)} icon`} onError={()=>setSource(null)}/>:<span className="app-icon app-icon-fallback" aria-hidden="true" style={{background:appColor(app)}}>{[...applicationLabel(app)].at(0)?.toUpperCase()||'?'}</span>;
}
