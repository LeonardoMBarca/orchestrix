/* UI localization only. Template values, user content and runtime identifiers stay intact. */
(() => {
  'use strict';
  const languages=Object.freeze([{id:'en',name:'English'},{id:'pt-BR',name:'Português'},{id:'es',name:'Español'}]);
  const catalog=globalThis.OrchestrixLocaleCatalog||{};
  const storageKey='orchestrix-prototype-language';
  const missing=new Set();
  let language='en';
  try {const saved=localStorage.getItem(storageKey);if(languages.some(item=>item.id===saved))language=saved;} catch {}
  const escape=value=>String(value).replace(/[&<>"']/g,char=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
  function translate(source,htmlSafe=false) {
    const raw=String(source);
    const key=raw.trim();
    if(!key||language==='pt-BR')return raw;
    const entry=catalog[key];
    if(!entry){if(/[A-Za-zÀ-ÿ]/.test(key))missing.add(key);return raw;}
    const value=entry[language];
    if(typeof value!=='string')throw new Error(`Missing ${language} UI translation: ${key}`);
    const start=raw.indexOf(key);
    return raw.slice(0,start)+(htmlSafe?escape(value):value)+raw.slice(start+key.length);
  }
  const t=source=>translate(source);
  // Values are opaque holes while static copy is translated, then inserted verbatim.
  const hole=/\uE000(\d+)\uE001/g;
  function pieces(source,htmlSafe=false) {
    return source.split(/(\uE000\d+\uE001)/).map(piece=>/^\uE000\d+\uE001$/.test(piece)?piece:translate(piece,htmlSafe)).join('');
  }
  function template(strings,values,markup) {
    const source=strings.reduce((result,part,index)=>result+part+(index<values.length?`\uE000${index}\uE001`:''),'');
    const translated=markup?source.split(/(<!--[\s\S]*?-->|<\/?[A-Za-z][^>]*>)/g).map(part=>{
      if(part.startsWith('<!--'))return part;
      if(part.startsWith('<'))return part.replace(/(\b(?:aria-label|aria-description|aria-valuetext|title|placeholder|alt)\s*=\s*)(["'])([\s\S]*?)\2/g,(_,prefix,quote,value)=>prefix+quote+pieces(value,true)+quote);
      return pieces(part,true);
    }).join(''):pieces(source);
    return translated.replace(hole,(_,index)=>String(values[Number(index)]));
  }
  const html=(strings,...values)=>template(strings,values,true);
  const text=(strings,...values)=>template(strings,values,false);
  function localizeStatic(root=document) {
    root.querySelectorAll('[data-i18n]').forEach(element=>{element.textContent=t(element.dataset.i18n);});
    for(const attribute of ['aria-label','title','placeholder','alt'])root.querySelectorAll(`[data-i18n-${attribute}]`).forEach(element=>{element.setAttribute(attribute,t(element.getAttribute(`data-i18n-${attribute}`)));});
    root.querySelectorAll('[data-language-picker]').forEach(select=>{select.value=language;});
  }
  function applyLanguage() {
    if(typeof document==='undefined')return;
    document.documentElement.lang=language;
    document.documentElement.dataset.language=language;
    localizeStatic();
  }
  function setLanguage(id,persist=true) {
    if(!languages.some(item=>item.id===id))return false;
    const previous=language;
    language=id;
    if(persist)try{localStorage.setItem(storageKey,id);}catch{}
    applyLanguage();
    if(previous!==language&&typeof document!=='undefined')document.dispatchEvent(new CustomEvent('orchestrix:language-change',{detail:{language,previous}}));
    return true;
  }
  globalThis.OrchestrixI18n=Object.freeze({t,html,text,languages,setLanguage,localizeStatic,get language(){return language;},missing:()=>[...missing],clearMissing:()=>missing.clear()});
  applyLanguage();
})();
