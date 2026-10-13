import {test,expect} from '@playwright/test';

test.beforeEach(async({page})=>{
  await page.addInitScript(()=>{
    const state = {state:'tracking',started_ts:Date.now(),updated_ts:Date.now(),backends:[{name:'window',state:'ready',error:null},{name:'idle',state:'ready',error:null}],current_window:{app:'Editor',title:'Document',pid:1},idle_seconds:0};
    (window as any).__fixtureStatus = state;
    (window as any).__TAURI__ = {core:{invoke:async(command:string,args:any)=>{
      if(command==='frontend_ready')return null;
      if(command==='start_tracking')return true;
      if(command==='api_request'){
        if(args.path==='/pause')state.state='paused';
        if(args.path==='/resume')state.state='tracking';
        if(args.path==='/status'||args.path==='/pause'||args.path==='/resume')return {...state};
        return [];
      }
      return null;
    }}};
  });
});

test('window and simulated tray share pause status; backend error is visible',async({page})=>{
  await page.goto('/');
  await expect(page.getByTestId('tracking-status')).toHaveText('Tracking');
  await page.getByRole('button',{name:'Pause tracking',exact:true}).click();
  await expect(page.getByTestId('tracking-status')).toHaveText('Paused');
  expect(await page.evaluate(()=> (window as any).__fixtureStatus.state)).toBe('paused');
  await page.evaluate(()=>{ (window as any).__fixtureStatus.state='tracking'; });
  await expect(page.getByTestId('tracking-status')).toHaveText('Tracking');
  await page.evaluate(()=>{
    (window as any).__fixtureStatus.state='error';
    (window as any).__fixtureStatus.backends=[{name:'window',state:'permission_required',error:'Grant Accessibility permission'}];
  });
  await expect(page.getByTestId('tracking-status')).toHaveText('Needs attention');
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await expect(page.locator('.backend').getByText('Grant Accessibility permission')).toBeVisible();
});

test('offline daemon has an actionable error state',async({page})=>{
  await page.addInitScript(()=>{ (window as any).__TAURI__.core.invoke = async()=>{throw new Error('Background service unavailable');}; });
  await page.goto('/');
  await expect(page.getByTestId('tracking-status')).toHaveText('Service offline');
  await expect(page.getByRole('button',{name:'Start tracking',exact:true})).toBeVisible();
});
