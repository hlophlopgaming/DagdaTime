export interface Range {start:number;end:number}
export interface ActivityEvent {id:number;bucket:string;start_ts:number;end_ts:number;data:Record<string,unknown>}
export interface Tag {id:number;name:string;color?:string|null;parent_id?:number|null}
export interface TagRange {id:number;tag_id:number;start_ts:number;end_ts:number;note?:string|null}
export interface Dataset {events:ActivityEvent[];tags:Tag[];ranges:TagRange[]}
export interface Segment extends Range {key:string;label:string;color:string;detail:string;app?:string}
export interface Track {name:string;segments:Segment[]}
export function dayBounds(date:string):Range {
  const start=new Date(`${date}T00:00:00`);const end=new Date(start);end.setDate(end.getDate()+1);
  return {start:start.getTime(),end:end.getTime()};
}
export function clip(value:Range,bounds:Range):Range|null {
  const start=Math.max(value.start,bounds.start),end=Math.min(value.end,bounds.end);
  return Number.isFinite(start)&&Number.isFinite(end)&&end>start?{start,end}:null;
}
function merged(values:Range[],bounds:Range):Range[] {
  const sorted=values.map(value=>clip(value,bounds)).filter((value):value is Range=>!!value).sort((a,b)=>a.start-b.start||a.end-b.end);
  const result:Range[]=[];
  for(const value of sorted){const last=result.at(-1);if(last&&value.start<=last.end)last.end=Math.max(last.end,value.end);else result.push({...value});}
  return result;
}
export function unionDuration(values:Range[],bounds:Range){return merged(values,bounds).reduce((sum,value)=>sum+value.end-value.start,0);}
function uncovered(base:Range[],covers:Range[],bounds:Range):Range[] {
  const result:Range[]=[],covered=merged(covers,bounds);
  for(const interval of merged(base,bounds)){
    let cursor=interval.start;
    for(const cover of covered){if(cover.end<=cursor)continue;if(cover.start>=interval.end)break;if(cover.start>cursor)result.push({start:cursor,end:Math.min(cover.start,interval.end)});cursor=Math.max(cursor,cover.end);if(cursor>=interval.end)break;}
    if(cursor<interval.end)result.push({start:cursor,end:interval.end});
  }
  return result;
}
export function intersectionDuration(a:Range[],b:Range[],bounds:Range){
  const first=merged(a,bounds),second=merged(b,bounds);let i=0,j=0,total=0;
  while(i<first.length&&j<second.length){const value=clip(first[i],second[j]);if(value)total+=value.end-value.start;if(first[i].end<second[j].end)i++;else j++;}
  return total;
}
export function summarize(values:Segment[],range:Range){
  const groups=new Map<string,Segment[]>();
  for(const value of values){if(!clip(value,range))continue;const group=groups.get(value.key)??[];group.push(value);groups.set(value.key,group);}
  return [...groups.entries()].map(([key,group])=>{const duration=unionDuration(group,range);return {key,label:group[0].label,color:group[0].color,duration,share:duration/Math.max(1,range.end-range.start)*100};}).sort((a,b)=>b.duration-a.duration||a.label.localeCompare(b.label));
}
export function pixelRange(view:Range,width:number,a:number,b:number):Range {
  const time=(x:number)=>Math.round(view.start+Math.max(0,Math.min(width,x))/Math.max(1,width)*(view.end-view.start));
  return {start:Math.min(time(a),time(b)),end:Math.max(time(a),time(b))};
}
export function zoomView(day:Range,view:Range,factor:number,anchor=(view.start+view.end)/2):Range {
  const span=Math.min(day.end-day.start,Math.max(60_000,(view.end-view.start)*factor));
  const start=Math.max(day.start,Math.min(day.end-span,anchor-span/2));return {start,end:start+span};
}
export function applicationLabel(app:string){return app.split(/[\\/]/).filter(Boolean).at(-1)||app;}
export function appColor(app:string){let hash=2166136261;for(const char of app){hash=Math.imul(hash^char.codePointAt(0)!,16777619);}return `hsl(${(hash>>>0)%360} 48% 56%)`;}
function text(value:unknown,fallback=''){return typeof value==='string'?value:fallback;}
export function tracks(data:Dataset,day:Range):Track[]{
  const result:Track[]=[{name:'Tags',segments:[]},{name:'Computer',segments:[]},{name:'Applications',segments:[]},{name:'Documents',segments:[]}];
  const add=(track:number,range:Range,key:string,label:string,color:string,detail:string)=>{const clipped=clip(range,day);if(clipped)result[track].segments.push({...clipped,key,label,color,detail,app:track===2?key:track===3?detail:undefined});};
  const tags=new Map(data.tags.map(tag=>[tag.id,tag]));
  for(const range of data.ranges){const tag=tags.get(range.tag_id);add(0,{start:range.start_ts,end:range.end_ts},String(range.tag_id),tag?.name??'Unknown tag',tag?.color??appColor(tag?.name??String(range.tag_id)),range.note??'');}
  for(const event of data.events){
    const range={start:event.start_ts,end:event.end_ts};
    if(event.bucket==='afk'){const state=text(event.data.status,'unknown');add(1,range,state,state==='active'?'Active':state==='away'?'Away':'Unobserved',state==='active'?'#5ea794':state==='away'?'#d3af66':'#7c8793','Computer usage');}
    if(event.bucket==='window'){
      const app=text(event.data.app,'Unknown application'),title=text(event.data.title,'Untitled window');
      add(2,range,app,applicationLabel(app),appColor(app),title);
      add(3,range,`${app}\0${title}`,title,appColor(app),app);
    }
    if(event.bucket==='web'){const url=text(event.data.url),title=text(event.data.title,url||'Browser tab');add(3,range,url||title,title,'#769fc3',url);}
  }
  const observed=result[1].segments.filter(segment=>segment.key==='active'||segment.key==='away');
  for(const range of uncovered(observed,result[0].segments,day))add(0,range,'untagged','Untagged','#a3adb8','Observed activity without a tag');
  for(const track of result)track.segments.sort((a,b)=>a.start-b.start||a.end-b.end);
  const known=result[1].segments.slice();
  for(let i=1;i<known.length;i++){if(known[i].start>known[i-1].end)add(1,{start:known[i-1].end,end:known[i].start},'unknown','Unobserved','#7c8793','Service stopped, paused, or computer unavailable');}
  result[1].segments.sort((a,b)=>a.start-b.start);
  return result;
}
export function continuousSpan(segment:Segment,values:Segment[]):Range {
  let start=segment.start,end=segment.end;
  for(let index=values.length-1;index>=0;index--){const value=values[index];if(value.key===segment.key&&value.end===start)start=value.start;}
  for(const value of values){if(value.key===segment.key&&value.start<=end&&value.end>=start)end=Math.max(end,value.end);}
  return {start,end};
}
export function duration(value:number){const seconds=Math.floor(Math.max(0,value)/1000),minutes=Math.floor(seconds/60),hours=Math.floor(minutes/60);return hours?`${hours}h ${minutes%60}m`:minutes?`${minutes}m`: `${seconds}s`;}
export function timeLabel(value:number){return new Date(value).toLocaleTimeString(undefined,{hour:'2-digit',minute:'2-digit'});}
export function fixture(date:string,count=5000):Dataset {
  const at=(hour:number)=>new Date(`${date}T${String(hour).padStart(2,'0')}:00:00`).getTime();
  const apps=['Editor','Browser','Terminal','Mail','Design'];const documents=['Project plan','Draft notes','Research','Design system','Weekly review','Prototype','Build output','Inbox'];
  const morning=Math.round(count*4/9);
  const events:ActivityEvent[]=Array.from({length:count},(_,index)=>{
    const before=index<morning,offset=before?index:index-morning,total=before?morning:count-morning,span=(before?4:5)*3_600_000,base=at(before?8:13);
    return {id:index+1,bucket:'window',start_ts:base+Math.round(offset/total*span),end_ts:base+Math.round((offset+1)/total*span),data:{app:apps[index%apps.length],title:documents[index%documents.length],pid:100+index%apps.length}};
  });
  events.push(...[{start:8,end:12,status:'active'},{start:12,end:13,status:'away'},{start:13,end:18,status:'active'}].map((value,index)=>({id:count+index+1,bucket:'afk',start_ts:at(value.start),end_ts:at(value.end),data:{status:value.status}})));
  return {events,tags:[{id:1,name:'Project Atlas',color:'#778dc3'},{id:2,name:'Studio Paper',color:'#5baf9d'}],ranges:[{id:1,tag_id:1,start_ts:at(8),end_ts:at(10),note:'Planning & research'},{id:2,tag_id:2,start_ts:at(13),end_ts:at(16),note:'Design & prototype'}]};
}
