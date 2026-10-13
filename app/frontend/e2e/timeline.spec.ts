import {test,expect} from '@playwright/test';

test('5000-event day renders, selects, zooms, scrolls and navigates',async({page})=>{
  await page.goto('/');
  await page.getByRole('button',{name:'View demo',exact:true}).click();
  await expect(page.getByText('Preview · 5,000 generated events')).toBeVisible();
  for(const name of ['Tags','Computer','Applications','Documents'])await expect(page.getByTestId(`track-${name}`)).toBeVisible();
  const canvas=page.getByTestId('timeline-canvas');
  await expect.poll(async()=>Number(await canvas.getAttribute('data-render-ms'))).toBeLessThan(100);
  const box=(await canvas.boundingBox())!;
  await page.getByRole('tab',{name:'Applications',exact:true}).click();
  const before=await page.getByTestId('summary-total').innerText();
  await page.mouse.move(box.x+box.width*.35,box.y+32+45*2+20);
  await page.mouse.down();await page.mouse.move(box.x+box.width*.55,box.y+32+45*2+20,{steps:12});await page.mouse.up();
  await expect(page.getByTestId('selection-caption')).not.toContainText('Whole day');
  await expect(page.getByTestId('summary-total')).not.toHaveText(before);
  await page.getByRole('button',{name:'Zoom in',exact:true}).click();
  await expect(page.getByTestId('zoom-caption')).not.toHaveText('Full day');
  const scroll=page.getByTestId('timeline-scroll');
  await scroll.evaluate(element=>{element.scrollLeft+=200;});
  expect(await scroll.evaluate(element=>element.scrollLeft)).toBeGreaterThan(0);
  await page.keyboard.press('Escape');await expect(page.getByTestId('selection-caption')).toContainText('Whole day');
  await page.screenshot({path:'test-results/day-view.png',fullPage:true});
  const date=await page.getByLabel('Selected date').inputValue();
  await page.getByRole('button',{name:'Next day',exact:true}).click();
  await expect(page.getByLabel('Selected date')).not.toHaveValue(date);
  await expect(page.getByText('Preview · 5,000 generated events')).toBeVisible();
});

test('keyboard zoom, search and track visibility work',async({page})=>{
  await page.goto('/');await page.getByRole('button',{name:'View demo',exact:true}).click();
  await page.keyboard.press('+');await expect(page.getByTestId('zoom-caption')).not.toHaveText('Full day');
  await page.getByLabel('Search activity').fill('Editor');
  await page.getByRole('tab',{name:'Applications',exact:true}).click();
  await expect(page.locator('.summary-table tbody tr')).toHaveCount(1);
  await page.getByText('Tracks',{exact:true}).click();
  await page.getByLabel('Show Documents').uncheck();
  await expect(page.getByTestId('track-Documents')).toHaveCount(0);
});
