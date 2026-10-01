(() => {
const hex=c=>{const m=c.match(/[\d.]+/g); if(!m) return c; if(m.length>3 && +m[3]===0) return ''; return '#'+m.slice(0,3).map(x=>(+x).toString(16).padStart(2,'0')).join('')+(m.length>3&&+m[3]<1?'@'+m[3]:'')};
const out=[];
document.querySelectorAll('body *').forEach(e=>{const r=e.getBoundingClientRect(); if(r.width===0||r.height===0) return; const cs=getComputedStyle(e); if(cs.visibility==='hidden') return;
 const bg=hex(cs.backgroundColor); const own=[...e.childNodes].filter(n=>n.nodeType===3&&n.textContent.trim()).map(n=>n.textContent.trim()).join(' ').slice(0,24);
 if(!own && !['INPUT','BUTTON','IMG','svg','H2'].includes(e.tagName) && e.getAttribute('role')!=='slider') return;
 out.push([e.tagName.toLowerCase()+(e.getAttribute('role')?'['+e.getAttribute('role')+']':''), own&&JSON.stringify(own), r.left.toFixed(2), r.top.toFixed(2), r.width.toFixed(2)+'x'+r.height.toFixed(2), own?cs.fontSize+'/'+cs.lineHeight+'/'+cs.fontWeight+'/'+hex(cs.color):'', bg&&'bg'+bg, cs.borderRadius!=='0px'?'r'+cs.borderRadius:''].filter(Boolean).join(' '))});
return out.join('\n');
})()
