import {useEffect,useState,useRef} from 'react';
import {invoke,request,localDate,shiftDate} from './client.ts';
import {statusView} from './shell.ts';
import type {Status} from './shell.ts';
import Day from './Day.tsx';

const screens=['Day','Week','Reports','Tags','Rules','Settings'];
const icons=['◷','▦','▤','◇','⌁','⚙'];

export default function App() {
  const [screen,setScreen]=useState('Day');
  const [date,setDate]=useState(localDate());
  const [status,setStatus]=useState<Status|null>(null);
  const [message,setMessage]=useState('');
  const [search,setSearch]=useState('');
  const [busy,setBusy]=useState(false);
  const [directory,setDirectory]=useState<{current:string,pending:string|null}|null>(null);
  useEffect(()=>{if(screen==='Settings')invoke<{current:string,pending:string|null}>('data_directory').then(setDirectory).catch(()=>{});},[screen]);
  async function chooseFolder(){try{setDirectory(await invoke('choose_data_directory'));}catch(error){setMessage(String(error));}}
  const searchRef=useRef<HTMLInputElement>(null);
  const view=statusView(status);
  useEffect(()=>{
    const settings=window.__TAURI__?.event?.listen('open-settings',()=>setScreen('Settings'));
    const errors=window.__TAURI__?.event?.listen('action-error',event=>setMessage(String(event.payload)));
    return()=>{settings?.then(stop=>stop());errors?.then(stop=>stop());};
  },[]);
  useEffect(()=>{
    let alive=true;
    const refresh=()=>request<Status>('/status').then(value=>{if(alive){setStatus(value);setMessage('');}}).catch(error=>{if(alive){setStatus(null);setMessage(String(error));}});
    refresh();
    const timer=setInterval(refresh,1000);
    invoke('frontend_ready').catch(()=>{});
    return()=>{alive=false;clearInterval(timer);};
  },[]);
  useEffect(()=>{
    const keyboard=(event:KeyboardEvent)=>{
      if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='f'){event.preventDefault();searchRef.current?.focus();return;}
      if((event.target as HTMLElement).closest('input,textarea,select'))return;
      if(event.key==='ArrowLeft')setDate(value=>shiftDate(value,-1));
      if(event.key==='ArrowRight')setDate(value=>shiftDate(value,1));
      if(event.key.toLowerCase()==='t')setDate(localDate());
    };
    window.addEventListener('keydown',keyboard);return()=>window.removeEventListener('keydown',keyboard);
  },[]);
  async function control(path:string,duration:number|null=null) {
    setBusy(true);
    try{setStatus(await request<Status>(path,'POST',path==='/pause'?{duration_ms:duration}:null));setMessage('');}
    catch(error){setMessage(String(error));}finally{setBusy(false);}
  }
  async function start(){setBusy(true);try{await invoke('start_tracking');setStatus(await request<Status>('/status'));setMessage('');}catch(error){setMessage(String(error));}finally{setBusy(false);}}
  return <div className="app">
    <aside className="sidebar">
      <div className="brand"><svg viewBox="0 0 32 32" aria-hidden="true"><circle cx="16" cy="16" r="12"/><path d="M16 8v8l6 4"/></svg><strong>DagdaTime</strong></div>
      <div className="nav-label">WORKSPACE</div>
      <nav>{screens.map((name,index)=><button key={name} onClick={()=>setScreen(name)} aria-current={screen===name?'page':undefined}><span aria-hidden="true">{icons[index]}</span>{name}</button>)}</nav>
      <div className="sidebar-bottom"><span className="local-dot"/>Local & private<div>Activity stays on this computer.</div></div>
    </aside>
    <main>
      <header className="toolbar">
        <div className="date-controls"><button aria-label="Previous day" onClick={()=>setDate(shiftDate(date,-1))}>‹</button><input aria-label="Selected date" type="date" value={date} onChange={event=>setDate(event.target.value||localDate())}/><button aria-label="Next day" onClick={()=>setDate(shiftDate(date,1))}>›</button><button onClick={()=>setDate(localDate())}>Today</button></div>
        <label className="search"><span aria-hidden="true">⌕</span><input ref={searchRef} aria-label="Search activity" value={search} onChange={event=>setSearch(event.target.value)} placeholder="Search activity"/><kbd>⌘ / Ctrl F</kbd></label>
        <span className={`status ${view.state}`} data-testid="tracking-status"><i/>{view.label}</span>
        {status&&<><button disabled={busy} aria-label={status.state==='paused'?'Resume tracking':'Pause tracking'} onClick={()=>control(status.state==='paused'?'/resume':'/pause')}>{status.state==='paused'?'Resume':'Pause'}</button>{status.state!=='paused'&&<select aria-label="Timed pause" disabled={busy} value="" onChange={event=>{if(event.target.value)control('/pause',Number(event.target.value));}}><option value="">Pause for…</option><option value="900000">15 minutes</option><option value="3600000">1 hour</option></select>}</>}
      </header>
      {(view.state==='error'||view.state==='offline'||message)&&<div className="banner" role="status"><span>{message||view.detail||'Tracking needs attention.'}</span>{!status?<button disabled={busy} onClick={start}>Start tracking</button>:<button onClick={()=>setScreen('Settings')}>Review permissions</button>}</div>}
      <section className="workspace">
        <div className="section-heading"><div><span className="eyebrow">YOUR TIME, IN PERSPECTIVE</span><h1>{screen==='Day'?'Day overview':screen}</h1></div><span className="date-caption">{new Date(`${date}T12:00:00`).toLocaleDateString(undefined,{weekday:'long',month:'long',day:'numeric',year:'numeric'})}</span></div>
        {screen==='Settings'?<div className="settings-grid">
          <article className="panel"><h2>Status & permissions</h2><p>Live status of the background service and activity collectors.</p>{status?.backends.map(backend=><div className="backend" key={backend.name}><div><strong>{backend.name==='window'?'Active window':'Idle detection'}</strong><span className={`badge ${backend.state}`}>{backend.state.replaceAll('_',' ')}</span></div>{backend.error&&<p>{backend.error}</p>}</div>)}{!status&&<p className="warning">{message||'Background service unavailable'}</p>}</article>
          <article className="panel"><h2>Tracking</h2><dl><dt>Current state</dt><dd>{view.label}</dd><dt>Application</dt><dd>{status?.current_window?.app||'—'}</dd><dt>Idle time</dt><dd>{status?.idle_seconds!=null?`${status.idle_seconds} seconds`:'Unavailable'}</dd><dt>Data</dt><dd>Local SQLite database</dd></dl><p>Closing the window keeps recording in the background. Use Quit in the tray to stop DagdaTime.</p></article>
          <article className="panel"><h2>Data folder</h2><p>Your activity database stays on this computer.</p><p>{directory?.current||'Default system application data folder'}</p><button onClick={chooseFolder}>Choose folder</button>{directory?.pending&&<p>Next launch: {directory.pending}. Restart DagdaTime to use this folder. Existing history stays in the current folder.</p>}</article>
        </div>:screen==='Day'?<Day date={date} search={search} status={status}/>:<article className="panel empty"><h2>{screen} is coming soon</h2><p>Your activity is kept locally while this view is being built.</p><button onClick={()=>setScreen('Day')}>Back to day</button></article>}
      </section>
      <footer><span><i className={`footer-dot ${view.state}`}/>{view.label}</span><span>DagdaTime · Private by design</span></footer>
    </main>
  </div>;
}
