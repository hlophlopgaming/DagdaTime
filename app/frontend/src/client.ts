declare global { interface Window { __TAURI__?:{core:{invoke:<T>(command:string,args?:Record<string,unknown>)=>Promise<T>},event?:{listen:(event:string,callback:(event:{payload:unknown})=>void)=>Promise<()=>void>}} } }
export function invoke<T>(command:string,args?:Record<string,unknown>):Promise<T> {
  if(!window.__TAURI__)return Promise.reject(new Error('Open DagdaTime desktop to start tracking.'));
  return window.__TAURI__.core.invoke<T>(command,args);
}
export function request<T>(path:string,method='GET',body:unknown=null) { return invoke<T>('api_request',{path,method,body}); }
export function localDate(date=new Date()) { return `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`; }
export function shiftDate(value:string,days:number) { const date=new Date(`${value}T12:00:00`);date.setDate(date.getDate()+days);return localDate(date); }
