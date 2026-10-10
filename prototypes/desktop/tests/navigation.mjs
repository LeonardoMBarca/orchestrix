export async function usePortuguese(page) {
  await page.addInitScript(()=>{
    window.__ORCHESTRIX_TEST_SCENARIO='seed';
    try{localStorage.setItem('orchestrix-prototype-language','pt-BR');}catch{}
  });
}

export async function openStudio(page) {
  const studio=page.locator('#studio-navigation');
  if(!await studio.evaluate(element=>element.open))await studio.locator('summary').click();
}

export async function openWorkspaceOptions(page) {
  await openSettings(page,'general');
}

export async function openSettings(page,tab='general') {
  await page.evaluate(tab=>OrchestrixSettings.open(tab),tab);
  await page.locator('#floating-settings').waitFor({state:'visible'});
}
