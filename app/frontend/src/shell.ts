export interface Backend { name:string; state:string; error?:string|null }
export interface Status { state:string; started_ts?:number; updated_ts?:number; paused_until?:number|null; backends:Backend[]; current_window?:{app:string;title:string;pid?:number}|null; idle_seconds?:number|null; error?:string|null }
export function statusView(status:Status|null) {
  const labels:Record<string,string> = {tracking:'Tracking',paused:'Paused',away:'Away',error:'Needs attention',starting:'Starting'};
  return {label:status ? labels[status.state]??'Needs attention' : 'Service offline',
    state:status?.state??'offline', detail:status?.error??status?.backends.find(backend=>backend.error)?.error??(status ? '' : 'The background service is not running.')};
}
