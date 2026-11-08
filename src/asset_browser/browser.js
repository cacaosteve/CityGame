'use strict';
const $ = id => document.getElementById(id);
const state = {catalog:null, category:'All assets', page:0, size:40, selected:null};
const images = new Map();
const number = n => n.toLocaleString();
const el = (tag, className, text) => {const node=document.createElement(tag); if(className)node.className=className; if(text!==undefined)node.textContent=text; return node;};
const packFor = id => state.catalog.packs.find(p=>p.id===id);
const replacements = asset => asset.variants.filter(v=>v.status!=='imported');
function status(asset){const variants=replacements(asset); return variants.some(v=>v.status==='reviewed')?'reviewed':variants.length?'draft':'missing';}
const statusLabel = value => ({reviewed:'Reviewed available',draft:'Draft available',missing:'Not started'})[value];
function loadImage(url){
  if(!images.has(url))images.set(url,new Promise((resolve,reject)=>{const img=new Image();img.onload=()=>resolve(img);img.onerror=()=>reject(new Error('Preview unavailable'));img.src=url;}));
  return images.get(url);
}
function preview(variant,zoom,large=false){
  const box=el('div','preview');
  if(variant.media_kind==='image'){
    const canvas=el('canvas');canvas.setAttribute('aria-label','Asset pixel preview');box.append(canvas);
    loadImage(variant.media_url).then(img=>{
      const [x,y,w,h]=variant.crop || [0,0,img.naturalWidth,img.naturalHeight];
      canvas.width=w;canvas.height=h;
      const scale=Math.max(.05,Math.min(large?6:4,(large?260:170)/w,(large?176:110)/h));
      canvas.style.width=`${w*scale}px`;canvas.style.height=`${h*scale}px`;
      const ctx=canvas.getContext('2d');ctx.imageSmoothingEnabled=false;ctx.drawImage(img,x,y,w,h,0,0,w,h);
    }).catch(()=>canvas.replaceWith(el('span','muted','Preview unavailable')));
  } else box.append(el('span','sound-glyph',variant.media_kind==='audio'?'▂▆▃▇▂':'♫'));
  if(zoom)box.append(el('span','zoom-badge',zoom));
  return box;
}
function updateStats(){
  const assets=state.catalog.assets;
  const replacementCount=assets.filter(a=>replacements(a).length).length;
  const families=new Map();for(const asset of assets){if(!families.has(asset.family))families.set(asset.family,[]);families.get(asset.family).push(asset);}
  const complete=[...families.values()].filter(group=>state.catalog.packs.some(p=>p.kind==='replacement' && group.every(a=>a.variants.some(v=>v.pack_id===p.id && v.status==='reviewed')))).length;
  const items=[['Catalog entries',number(assets.length),'Across all loaded packs'],['Source packs',number(state.catalog.packs.length),'Imported + authored'],['Replacement coverage',`${assets.length?Math.round(replacementCount/assets.length*100):0}%`,`${number(replacementCount)} entries have artwork`],['Reviewed families',number(complete),`Of ${number(families.size)} observed families`]];
  $('stats').replaceChildren(...items.map(([label,value,note])=>{const node=el('div','stat');node.append(el('span','stat-label',label),el('span','stat-value',value),el('span','stat-note',note));return node;}));
}
function initialize(){
  const categories=['All assets',...new Set(state.catalog.assets.map(a=>a.category).sort())];
  for(const name of categories){
    const button=el('button',name===state.category?'active':'',name);button.type='button';
    button.append(el('span','',number(name==='All assets'?state.catalog.assets.length:state.catalog.assets.filter(a=>a.category===name).length)));
    button.addEventListener('click',()=>{state.category=name;state.page=0;for(const b of $('categories').children){b.classList.toggle('active',b===button);b.setAttribute('aria-current',b===button?'true':'false');}render();});
    if(name===state.category)button.setAttribute('aria-current','true');$('categories').append(button);
  }
  $('pack-summary').textContent=state.catalog.packs.map(p=>p.name).join(' · ');
  for(const pack of state.catalog.packs){
    const option=el('option','',pack.name);option.value=pack.id;$('pack-filter').append(option);
    const node=el('article','pack');node.append(el('h3','',pack.name),el('code','',pack.root));
    const list=el('ul');for(const note of pack.notes)list.append(el('li','',note));
    if(pack.author)list.append(el('li','',`${pack.author} · ${pack.license}`));
    node.append(list);$('packs').append(node);
  }
  if(state.catalog.warnings.length){const warning=el('details','warning');warning.append(el('summary','',`${state.catalog.warnings.length} import warnings — some records were skipped`));const list=el('ul');for(const text of state.catalog.warnings)list.append(el('li','',text));warning.append(list);$('warnings').append(warning);}
  updateStats();render();
}
function filtered(){
  const query=$('search').value.trim().toLowerCase(),pack=$('pack-filter').value,zoom=$('zoom-filter').value,replacementState=$('status-filter').value;
  return state.catalog.assets.filter(a=>(state.category==='All assets'||a.category===state.category) && (!pack||a.variants.some(v=>v.pack_id===pack)) && (!zoom||a.zoom===zoom) && (!replacementState||status(a)===replacementState) && (!query||`${a.name} ${a.key} ${a.category} ${a.family} ${a.sprite_id??''}`.toLowerCase().includes(query))).sort((a,b)=>a.category.localeCompare(b.category)||a.name.localeCompare(b.name,undefined,{numeric:true})||(b.sprite_id??0)-(a.sprite_id??0));
}
function render(){
  if(!state.catalog)return;
  const assets=filtered(),pages=Math.max(1,Math.ceil(assets.length/state.size));state.page=Math.min(state.page,pages-1);
  $('collection-title').textContent=state.category;
  $('result-count').textContent=`${number(assets.length)} entries · select an asset to compare editions and view its requirements`;
  const fragment=document.createDocumentFragment();
  for(const asset of assets.slice(state.page*state.size,(state.page+1)*state.size)){
    const packId=$('pack-filter').value||asset.variants[0].pack_id;
    const variant=[...asset.variants].reverse().find(v=>v.pack_id===packId);
    const card=el('button','asset-card');card.type='button';card.setAttribute('aria-label',`Inspect ${asset.name}, ${asset.zoom||asset.category}, ${asset.key}`);
    card.append(preview(variant,asset.zoom));const body=el('div','card-body');
    body.append(el('div','card-name',asset.name),el('code','card-key',asset.key));
    const packCount=new Set(asset.variants.map(v=>v.pack_id)).size;
    const bottom=el('div','card-bottom');bottom.append(el('span',`state ${status(asset)}`,statusLabel(status(asset))),el('span','',`${packCount} pack${packCount===1?'':'s'}`));body.append(bottom);card.append(body);
    card.addEventListener('click',()=>openDetail(asset));fragment.append(card);
  }
  $('grid').replaceChildren(fragment);$('empty').hidden=assets.length>0;
  $('page-status').textContent=`Page ${state.page+1} of ${pages}`;$('previous').disabled=state.page===0;$('next').disabled=state.page+1>=pages;
}
function stopAudio(){for(const audio of $('detail').querySelectorAll('audio'))audio.pause();}
function openDetail(asset){
  stopAudio();state.selected=asset;
  $('detail-title').textContent=asset.name;$('detail-category').textContent=asset.category.toUpperCase();$('detail-key').textContent=asset.key;
  const comparisons=[];
  for(const variant of asset.variants){
    const node=el('article','comparison');node.append(preview(variant,asset.zoom,true));
    const info=el('div','comparison-info');info.append(el('h3','',packFor(variant.pack_id).name));
    info.append(el('p','',`${variant.status==='imported'?'Original import':`Replacement · ${variant.status}`}${variant.size?` · ${variant.size[0]} × ${variant.size[1]} px`:''}`));
    if(variant.media_kind==='audio'){const player=el('audio');player.controls=true;player.preload='none';player.src=variant.media_url;player.addEventListener('play',()=>{for(const other of $('detail').querySelectorAll('audio'))if(other!==player)other.pause();});info.append(player);}
    if(variant.media_kind==='midi')info.append(el('p','','MIDI inventoried; synthesis is not available in this browser.'));
    if(variant.source_id!==null)info.append(el('p','',`Source ID: ${variant.source_id}${variant.source_offset!==null?` · byte offset ${variant.source_offset}`:''}`));
    if(variant.source_record!==null)info.append(el('p','',`Atlas record: ${variant.source_record}`));
    if(variant.logical_size)info.append(el('p','',`Logical canvas: ${variant.logical_size[0]} × ${variant.logical_size[1]} px`));
    if(variant.anchor)info.append(el('p','',`Declared anchor: ${variant.anchor.join(', ')} ${variant.logical_size?'logical ':''}px`));
    if(variant.animation)info.append(el('p','',`Animation / playback: ${variant.animation}`));
    info.append(el('code','',variant.file));node.append(info);comparisons.push(node);
  }
  if(!replacements(asset).length){const missing=el('article','comparison');missing.append(el('div','missing','Our artwork · not started'));const info=el('div','comparison-info');info.append(el('h3','','A new interpretation belongs here.'),el('p','','Use the technical brief to author an original asset. Add it in a replacement pack when it is ready for review.'));missing.append(info);comparisons.push(missing);}
  $('comparisons').replaceChildren(...comparisons);
  $('contract').replaceChildren(...[['Footprint',asset.footprint?`${asset.footprint.join(' × ')} tiles (engine table)`:'Needs mapping'],['Zoom / family',`${asset.zoom||'Not applicable'} · ${asset.family}`],['Animation',asset.animation]].map(([label,value])=>{const node=el('div');node.append(el('span','',label),document.createTextNode(value));return node;}));
  $('questions').replaceChildren(...asset.contract_notes.map(note=>el('li','',note)));
  const family=state.catalog.assets.filter(a=>a.family===asset.family).sort((a,b)=>(a.sprite_id??0)-(b.sprite_id??0));
  $('family').replaceChildren(...family.map(a=>{const b=el('button',a.key===asset.key?'selected':'',`${a.zoom||a.name} · ${statusLabel(status(a))}`);b.type='button';b.addEventListener('click',()=>openDetail(a));return b;}));
  if(!$('detail').open)$('detail').showModal();else $('detail').scrollTop=0;
}
for(const id of ['search','pack-filter','zoom-filter','status-filter'])$(id).addEventListener(id==='search'?'input':'change',()=>{state.page=0;render();});
$('previous').addEventListener('click',()=>{state.page--;render();});$('next').addEventListener('click',()=>{state.page++;render();});
$('close-detail').addEventListener('click',()=>$('detail').close());$('detail').addEventListener('close',stopAudio);
$('detail').addEventListener('click',event=>{if(event.target===$('detail')){const r=$('detail').getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)$('detail').close();}});
fetch('/api/catalog').then(response=>{if(!response.ok)throw new Error(`Catalog request failed (${response.status})`);return response.json();}).then(catalog=>{state.catalog=catalog;initialize();}).catch(error=>{$('result-count').textContent='Unable to load collection';$('warnings').append(el('div','warning',error.message));});
