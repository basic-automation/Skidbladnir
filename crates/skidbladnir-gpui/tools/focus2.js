(() => { const o=c=>[c.outlineStyle, c.outlineWidth, c.outlineColor, c.outlineOffset, c.borderRadius, c.backgroundColor, c.borderLeftColor].join(' | ');
 const out={};
 const focusRead=(name, el, styled) => { el.focus(); out[name]=document.activeElement===el ? o(getComputedStyle(styled||el)) : 'not focused'; el.blur(); };
 const r=document.querySelector('main button[role=radio]'); focusRead('radio-item(label)', r, r.closest('label'));
 const rr=document.querySelector('nav button[role=radio]'); focusRead('rail-item(label)', rr, rr.closest('label'));
 const q=[...document.querySelectorAll('aside button')].find(b=>b.textContent.includes('Queue')); focusRead('queue', q);
 const d=[...document.querySelectorAll('aside button')].find(b=>b.textContent.includes('Destination')); focusRead('destination', d);
 const p=[...document.querySelectorAll('aside li button')][0]; focusRead('preset', p);
 const x=document.querySelector('aside li button[aria-label^=Delete]'); focusRead('preset-delete', x);
 const add=document.querySelector('aside button[aria-label^=Save]'); focusRead('preset-add', add);
 const conv=[...document.querySelectorAll('main button')].find(b=>b.textContent.includes('Convert')); conv.disabled=false; focusRead('convert', conv); conv.disabled=true;
 const disc=document.querySelector('button[data-disclosure]'); focusRead('disclosure', disc);
 return JSON.stringify(out,null,1) })()
