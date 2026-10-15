import type {KeyboardEvent} from 'react';
export function modalKey(event:KeyboardEvent<HTMLElement>,close:()=>void){
 if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();return;}
 if(event.key!=='Tab')return;
 const controls=[...event.currentTarget.querySelectorAll<HTMLElement>('button:not([disabled]),input:not([disabled]),textarea:not([disabled]),select:not([disabled]),[tabindex="0"]')].filter(node=>node.getClientRects().length>0);
 if(!controls.length){event.preventDefault();return;}
 const first=controls[0],last=controls.at(-1)!;
 if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}
 else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
}
