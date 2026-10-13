import {useEffect,useState} from 'react';
import {invoke} from './client.ts';
import {appColor,applicationLabel} from './timeline.ts';
const cache=new Map<string,Promise<string|null>>();
export default function AppIcon({app}:{app:string}){
  const [source,setSource]=useState<string|null>(null);
  useEffect(()=>{
    let alive=true;setSource(null);
    if(!cache.has(app))cache.set(app,invoke<Record<string,string|null>>('application_icons',{apps:[app]}).then(value=>value?.[app]??null).catch(()=>null));
    cache.get(app)!.then(value=>{if(alive)setSource(value);});
    return()=>{alive=false;};
  },[app]);
  return source?<img className="app-icon" src={source} alt={`${applicationLabel(app)} icon`} onError={()=>setSource(null)}/>:<span className="app-icon app-icon-fallback" aria-hidden="true" style={{background:appColor(app)}}>{[...applicationLabel(app)].at(0)?.toUpperCase()||'?'}</span>;
}
