import {memo,useEffect,useLayoutEffect,useMemo,useRef,useState} from 'react';
import {request} from './client.ts';
import {usePolling,fingerprint} from './polling.ts';
import AppIcon from './AppIcon.tsx';
import {dayBounds,tracks as buildTracks,summarize,unionDuration,intersectionDuration,clip,pixelRange,zoomView,continuousSpan,fixture,duration,timeLabel} from './timeline.ts';
import type {Dataset,Range,Segment,Track,ActivityEvent,Tag,TagRange} from './timeline.ts';

const ROW=45,RULER=32;
const empty:Dataset={events:[],tags:[],ranges:[]};

function Timeline({all,day,selection,onSelect,highlight}:{all:Track[];day:Range;selection:Range|null;onSelect:(value:Range|null)=>void;highlight:string|null}){
  const [order,setOrder]=useState(all.map(track=>track.name));
  const [hidden,setHidden]=useState<string[]>([]);
  const [view,setView]=useState(day);
  const [width,setWidth]=useState(700);
  const [hover,setHover]=useState<{segment:Segment;x:number;y:number}|null>(null);
  const scroll=useRef<HTMLDivElement>(null),canvas=useRef<HTMLCanvasElement>(null);
  const anchor=useRef<{x:number;old:Range|null;shift:boolean}|null>(null);
  const visible=order.filter(name=>!hidden.includes(name)).map(name=>all.find(track=>track.name===name)!);
  const span=view.end-view.start;
  const virtualWidth=width*(day.end-day.start)/span;
  function zoom(factor:number){setView(current=>zoomView(day,current,factor,selection?(selection.start+selection.end)/2:undefined));}
  useEffect(()=>{
    const observer=new ResizeObserver(entries=>setWidth(Math.max(1,entries[0].contentRect.width)));
    if(scroll.current)observer.observe(scroll.current);return()=>observer.disconnect();
  },[]);
  useLayoutEffect(()=>{if(scroll.current)scroll.current.scrollLeft=(view.start-day.start)/span*width;},[span,day.start,width]);
  useEffect(()=>{
    const listener=(event:KeyboardEvent)=>{
      if((event.target as HTMLElement).closest('input,textarea,select'))return;
      if(['+','=','-'].includes(event.key)){event.preventDefault();zoom(event.key==='-'?2:.5);}
      if(event.key==='Escape')onSelect(null);
    };
    window.addEventListener('keydown',listener);return()=>window.removeEventListener('keydown',listener);
  },[day.start,day.end,selection]);
  useEffect(()=>{
    const element=canvas.current;if(!element)return;
    const started=performance.now(),ratio=window.devicePixelRatio||1,height=RULER+visible.length*ROW;
    element.width=Math.ceil(width*ratio);element.height=Math.ceil(height*ratio);
    const context=element.getContext('2d')!;context.scale(ratio,ratio);
    const dark=window.matchMedia('(prefers-color-scheme:dark)').matches;
    const line=dark?'#2a3945':'#edf0f3',muted=dark?'#92a5b3':'#8895a4';
    context.clearRect(0,0,width,height);context.font='11px system-ui';context.textBaseline='middle';
    const x=(time:number)=>(time-view.start)/span*width;
    const tick=span>12*3_600_000?2*3_600_000:span>4*3_600_000?3_600_000:span>3_600_000?30*60_000:span>15*60_000?5*60_000:60_000;
    for(let time=Math.ceil(view.start/tick)*tick;time<view.end;time+=tick){const left=x(time);context.strokeStyle=line;context.beginPath();context.moveTo(left,RULER);context.lineTo(left,height);context.stroke();context.fillStyle=muted;context.fillText(timeLabel(time),left+5,15);}
    visible.forEach((track,index)=>{
      const top=RULER+index*ROW;context.strokeStyle=line;context.beginPath();context.moveTo(0,top+ROW);context.lineTo(width,top+ROW);context.stroke();
      let previousPixel=-1;
      for(const segment of track.segments){const range=clip(segment,view);if(!range)continue;const left=x(range.start),right=x(range.end),size=right-left;
        // At day scale, collapse subpixel blocks to a representative pixel; summary math stays exact.
        if(size<1&&Math.floor(left)===previousPixel)continue;previousPixel=Math.floor(left);
        context.globalAlpha=highlight&&segment.key!==highlight? .25:1;
        context.fillStyle=segment.color;context.fillRect(left,top+9,Math.max(1,size),ROW-18);
        if(size>78){context.save();context.beginPath();context.rect(left+5,top+9,size-10,ROW-18);context.clip();context.fillStyle='#ffffff';context.fillText(segment.label,left+7,top+ROW/2);context.restore();}
      }
      context.globalAlpha=1;
    });
    if(selection){const range=clip(selection,view);if(range){const left=x(range.start),right=x(range.end);context.fillStyle='#278f8530';context.fillRect(left,RULER,right-left,height-RULER);context.strokeStyle='#278f85';context.strokeRect(left+.5,RULER+.5,Math.max(1,right-left-1),height-RULER-1);}}
    element.dataset.renderMs=String(performance.now()-started);
  },[all,order,hidden,view,width,selection,highlight]);
  function hit(x:number,y:number){const row=Math.floor((y-RULER)/ROW);const time=view.start+x/width*span;return visible[row]?.segments.findLast(segment=>segment.start<=time&&segment.end>time);}
  function coordinates(event:React.PointerEvent<HTMLCanvasElement>){const rect=event.currentTarget.getBoundingClientRect();return {x:event.clientX-rect.left,y:event.clientY-rect.top};}
  function extend(value:Range,old:Range|null){return old?{start:Math.min(old.start,value.start),end:Math.max(old.end,value.end)}:value;}
  function reorder(index:number,delta:number){const next=[...order];[next[index],next[index+delta]]=[next[index+delta],next[index]];setOrder(next);}
  return <article className="panel timeline-panel">
    <div className="timeline-toolbar"><span><b>Activity timeline</b><small>Drag to select · Shift-click to extend</small></span><div className="timeline-options">
      <details><summary>Tracks</summary><div className="track-menu">{order.map((name,index)=><div key={name}><label><input type="checkbox" aria-label={`Show ${name}`} checked={!hidden.includes(name)} onChange={()=>setHidden(current=>current.includes(name)?current.filter(value=>value!==name):[...current,name])}/>{name}</label><button aria-label={`Move ${name} up`} disabled={index===0} onClick={()=>reorder(index,-1)}>↑</button><button aria-label={`Move ${name} down`} disabled={index===order.length-1} onClick={()=>reorder(index,1)}>↓</button></div>)}</div></details>
      <button aria-label="Zoom out" onClick={()=>zoom(2)} disabled={span>=day.end-day.start}>−</button><span data-testid="zoom-caption">{span===day.end-day.start?'Full day':duration(span)}</span><button aria-label="Zoom in" onClick={()=>zoom(.5)} disabled={span<=60_000}>+</button><button onClick={()=>setView(day)}>Fit day</button>
    </div></div>
    <div className="timeline-grid"><div className="track-labels"><div className="ruler-label">LOCAL TIME</div>{visible.map(track=><div data-testid={`track-${track.name}`} key={track.name}><span className={`track-mark ${track.name.toLowerCase()}`}/>{track.name}</div>)}</div>
    <div className="timeline-scroll" data-testid="timeline-scroll" ref={scroll} onScroll={event=>{const start=day.start+event.currentTarget.scrollLeft/Math.max(1,width)*span;setView({start,end:start+span});setHover(null);}}>
      <div style={{width:virtualWidth,minHeight:RULER+visible.length*ROW}}>
        <canvas ref={canvas} data-testid="timeline-canvas" aria-label="Activity timeline. Drag to select a time range; use zoom buttons and the horizontal scrollbar." role="img" style={{width,height:RULER+visible.length*ROW}}
          onPointerDown={event=>{if(event.button!==0)return;const {x}=coordinates(event);anchor.current={x,old:selection,shift:event.shiftKey};event.currentTarget.setPointerCapture(event.pointerId);setHover(null);}}
          onPointerMove={event=>{const {x,y}=coordinates(event);if(anchor.current){if(Math.abs(x-anchor.current.x)>3)onSelect(extend(pixelRange(view,width,anchor.current.x,x),anchor.current.shift?anchor.current.old:null));return;}const segment=hit(x,y);setHover(segment?{segment,x:event.clientX,y:event.clientY}:null);}}
          onPointerUp={event=>{const down=anchor.current;if(!down)return;const {x,y}=coordinates(event);if(Math.abs(x-down.x)<=3){const segment=hit(x,y);const value=segment?{start:segment.start,end:segment.end}:pixelRange(view,width,x,x);onSelect(down.shift&&down.old?extend(value,down.old):segment?value:null);}anchor.current=null;event.currentTarget.releasePointerCapture(event.pointerId);}}
          onPointerCancel={()=>{anchor.current=null;}}
          onPointerLeave={()=>setHover(null)}
          onDoubleClick={event=>{const rect=event.currentTarget.getBoundingClientRect();const x=event.clientX-rect.left,y=event.clientY-rect.top,segment=hit(x,y);if(segment){const row=Math.floor((y-RULER)/ROW);onSelect(continuousSpan(segment,visible[row].segments));}}}/>
      </div>
    </div></div>
    <div className="timeline-legend"><span><i style={{background:'#5ea794'}}/>Active</span><span><i style={{background:'#d3af66'}}/>Away</span><span><i style={{background:'#7c8793'}}/>Unobserved</span><small>Unobserved gaps may include pauses or time with the computer off.</small></div>
    {hover&&<div className="hover-card" style={{left:Math.min(window.innerWidth-300,hover.x+12),top:Math.min(window.innerHeight-145,hover.y+14)}}><strong>{hover.segment.app&&<AppIcon app={hover.segment.app}/>} {hover.segment.label}</strong><p>{hover.segment.detail}</p><span>{timeLabel(hover.segment.start)}–{timeLabel(hover.segment.end)} · {duration(hover.segment.end-hover.segment.start)}</span></div>}
  </article>;
}

// Memoized: App re-renders every second with a fresh status; the day view only needs
// to know whether the service is online and when it started.
export default memo(function Day({date,search,online,started}:{date:string;search:string;online:boolean;started?:number}){
  const [data,setData]=useState<Dataset>(empty),[demo,setDemo]=useState(false),[error,setError]=useState(''),[loading,setLoading]=useState(true);
  const [selection,setSelection]=useState<Range|null>(null),[tab,setTab]=useState('Tags'),[highlight,setHighlight]=useState<string|null>(null),[limit,setLimit]=useState(50);
  const day=useMemo(()=>dayBounds(date),[date]);
  useEffect(()=>{setSelection(null);setHighlight(null);setData(empty);},[date]);
  const loaded=useRef<{key:string;hash:number}|null>(null),generation=useRef(0);
  function load(){
    const key=`${day.start}-${day.end}`,current=++generation.current;
    return Promise.all([request<ActivityEvent[]>(`/events?from=${day.start}&to=${day.end}`),request<Tag[]>('/tags'),request<TagRange[]>('/tag_ranges')]).then(([events,tags,ranges])=>{
      if(current!==generation.current)return;
      // Skip identical polls: a new object would rebuild every track and redraw the canvas.
      const hash=fingerprint(events,tags,ranges);
      if(loaded.current?.key!==key||loaded.current.hash!==hash){loaded.current={key,hash};setData({events,tags,ranges});}
      setError('');setLoading(false);
    }).catch(reason=>{if(current===generation.current){setError(String(reason));setLoading(false);}});
  }
  useEffect(()=>{
    loaded.current=null;
    if(demo){generation.current++;setData(fixture(date));setError('');setLoading(false);return;}
    setLoading(true);load();
    return()=>{generation.current++;};
  },[date,demo,day.start,day.end]);
  // Only today's data still changes; past days are loaded once.
  const live=!demo&&day.start<=Date.now()&&Date.now()<day.end;
  usePolling(()=>{load();},10_000,live);
  const all=useMemo(()=>buildTracks(data,day),[data,day]);
  const filtered=useMemo(()=>search.trim()?all.map(track=>({...track,segments:track.segments.filter(segment=>`${segment.label} ${segment.detail}`.toLowerCase().includes(search.toLowerCase()))})):all,[all,search]);
  const range=selection??day;
  const selected=filtered.find(track=>track.name===tab)!;
  const rows=useMemo(()=>summarize(selected.segments,range),[selected,range]);
  const summaryRows=rows;
  const {active,away,tagged,untagged}=useMemo(()=>{
    const observed=all[1].segments.filter(segment=>segment.key==='active'||segment.key==='away');
    const taggedSegments=all[0].segments.filter(segment=>segment.key!=='untagged');
    return {active:unionDuration(all[1].segments.filter(segment=>segment.key==='active'),day),away:unionDuration(all[1].segments.filter(segment=>segment.key==='away'),day),tagged:unionDuration(taggedSegments,day),
      untagged:Math.max(0,unionDuration(observed,day)-intersectionDuration(observed,taggedSegments,day))};
  },[all,day]);
  function select(value:Range|null){setSelection(value);setHighlight(null);setLimit(50);}
  function toggleDemo(){setDemo(value=>!value);setSelection(null);setHighlight(null);setData(empty);}
  return <div className="day-view">
    <div className="day-metrics"><div><span>ACTIVE TIME</span><strong>{duration(active)}</strong></div><div><span>AWAY TIME</span><strong>{duration(away)}</strong></div><div><span>TAGGED TIME</span><strong>{duration(tagged)}</strong></div><div><span>UNTAGGED ACTIVITY</span><strong>{duration(untagged)}</strong></div><button className="demo-button" onClick={toggleDemo}>{demo?'Return to live data':'View demo'}</button></div>
    {demo&&<div className="preview-label">Preview · 5,000 generated events<span>Sample data is never saved or mixed with your activity.</span></div>}
    {error&&!demo&&<div className="data-note">Activity data is unavailable. {error}</div>}
    <Timeline key={date} all={filtered} day={day} selection={selection} onSelect={select} highlight={highlight}/>
    {!data.events.length&&!data.ranges.length&&<div className="empty-day">{loading?'Loading your activity…':online?`No activity for this day yet. ${started?`Tracking started at ${timeLabel(started)}.`:''}`:'Start tracking to see your day, or explore the demo.'}</div>}
    <article className="panel summary-panel"><div className="selection-bar"><div><span className="selection-icon">⌗</span><strong data-testid="selection-caption">{selection?`Selection ${timeLabel(selection.start)}–${timeLabel(selection.end)} · ${duration(selection.end-selection.start)}`:'Whole day'}</strong>{selection&&<button className="clear-selection" onClick={()=>select(null)}>Clear selection</button>}</div><button disabled title="Time tagging is coming soon">Tag selection ↵</button></div>
      <div className="summary-header"><div role="tablist" aria-label="Selection summary">{['Tags','Applications','Documents'].map(name=><button key={name} role="tab" aria-selected={tab===name} onClick={()=>{setTab(name);setHighlight(null);setLimit(50);}}>{name}</button>)}</div><span><b data-testid="summary-total">{duration(summaryRows.reduce((sum,row)=>sum+row.duration,0))}</b> observed in this view</span></div>
      <table className="summary-table"><thead><tr><th>{tab==='Tags'?'Project / tag':tab==='Applications'?'Application':'Document / URL'}</th><th>Duration</th><th>Share of time range</th><th aria-label="Proportion"/></tr></thead><tbody>{summaryRows.slice(0,limit).map(row=><tr key={row.key} onClick={()=>setHighlight(highlight===row.key?null:row.key)} className={highlight===row.key?'highlighted':''}><td><button onClick={event=>{event.stopPropagation();setHighlight(highlight===row.key?null:row.key);}}>{tab==='Applications'||tab==='Documents'?<AppIcon app={row.key.split('\0')[0]}/>:<i style={{background:row.color}}/>}{row.label}</button></td><td>{duration(row.duration)}</td><td>{row.share.toFixed(1)}%</td><td><div className="share-bar"><i style={{width:`${Math.min(100,row.share)}%`,background:row.color}}/></div></td></tr>)}</tbody></table>
      {!summaryRows.length&&<p className="summary-empty">{search?'No matching activity in this range.':'No activity in this range yet.'}</p>}
      {summaryRows.length>limit&&<button className="show-more" onClick={()=>setLimit(value=>value+50)}>Show 50 more · {summaryRows.length-limit} remaining</button>}
      <div className="summary-footnote">Durations use interval unions; overlaps within the same item are counted once. Shares refer to the selected time range.</div>
    </article>
  </div>;
});
